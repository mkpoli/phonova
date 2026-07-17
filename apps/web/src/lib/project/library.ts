import type { LibraryGroup, LibraryNode } from '$lib/core/types';

/** A library node flattened for display, with its tree position. */
export interface LibraryRow {
  /** `media:<id>` or `group:<id>`, unique across the flattened tree. */
  key: string;
  depth: number;
  node: LibraryNode;
}

export function isGroup(node: LibraryNode): node is { Group: LibraryGroup } {
  return 'Group' in node;
}

export function nodeKey(node: LibraryNode): string {
  return isGroup(node) ? `group:${node.Group.id}` : `media:${node.Media}`;
}

/** Depth-first flattening; a collapsed group's children are omitted. */
export function flattenTree(
  nodes: LibraryNode[],
  collapsed: ReadonlySet<number>,
  depth = 0
): LibraryRow[] {
  const out: LibraryRow[] = [];
  for (const node of nodes) {
    out.push({ key: nodeKey(node), depth, node });
    if (isGroup(node) && !collapsed.has(node.Group.id)) {
      out.push(...flattenTree(node.Group.children, collapsed, depth + 1));
    }
  }
  return out;
}

/** Every recording id under `nodes`, depth-first. */
export function mediaIdsOf(nodes: LibraryNode[]): number[] {
  const out: number[] = [];
  const walk = (list: LibraryNode[]) => {
    for (const node of list) {
      if (isGroup(node)) walk(node.Group.children);
      else out.push(node.Media);
    }
  };
  walk(nodes);
  return out;
}

/** Highest group id anywhere in the tree, or 0 when there are none. */
function maxGroupId(nodes: LibraryNode[]): number {
  let max = 0;
  for (const node of nodes) {
    if (isGroup(node)) {
      max = Math.max(max, node.Group.id, maxGroupId(node.Group.children));
    }
  }
  return max;
}

/** A group id not yet used anywhere in `nodes`. */
export function nextGroupId(nodes: LibraryNode[]): number {
  return maxGroupId(nodes) + 1;
}

/** Creates a new empty group at the root, or nested inside `parentGroupId`. */
export function createGroup(
  nodes: LibraryNode[],
  name: string,
  parentGroupId: number | null
): LibraryNode[] {
  const group: LibraryNode = { Group: { id: nextGroupId(nodes), name, children: [] } };
  if (parentGroupId === null) return [...nodes, group];
  return mapGroups(nodes, parentGroupId, (g) => ({ ...g, children: [...g.children, group] }));
}

/** Renames the group identified by `groupId`, wherever it nests. */
export function renameGroup(nodes: LibraryNode[], groupId: number, name: string): LibraryNode[] {
  return mapGroups(nodes, groupId, (g) => ({ ...g, name }));
}

/** Applies `fn` to the group matching `groupId`, recursing into every group's children. */
function mapGroups(
  nodes: LibraryNode[],
  groupId: number,
  fn: (group: LibraryGroup) => LibraryGroup
): LibraryNode[] {
  return nodes.map((node) => {
    if (!isGroup(node)) return node;
    if (node.Group.id === groupId) return { Group: fn(node.Group) };
    return { Group: { ...node.Group, children: mapGroups(node.Group.children, groupId, fn) } };
  });
}

/** Dissolves a group, splicing its children into its parent at its position. */
export function dissolveGroup(nodes: LibraryNode[], groupId: number): LibraryNode[] {
  const out: LibraryNode[] = [];
  for (const node of nodes) {
    if (isGroup(node) && node.Group.id === groupId) {
      out.push(...node.Group.children);
      continue;
    }
    if (isGroup(node)) {
      out.push({ Group: { ...node.Group, children: dissolveGroup(node.Group.children, groupId) } });
      continue;
    }
    out.push(node);
  }
  return out;
}

/** Removes a node (media leaf or group, with its subtree) from wherever it sits. */
function removeNode(nodes: LibraryNode[], key: string): { nodes: LibraryNode[]; removed: LibraryNode | null } {
  let removed: LibraryNode | null = null;
  const out: LibraryNode[] = [];
  for (const node of nodes) {
    if (removed === null && nodeKey(node) === key) {
      removed = node;
      continue;
    }
    if (isGroup(node)) {
      const inner = removeNode(node.Group.children, key);
      if (inner.removed) removed = inner.removed;
      out.push({ Group: { ...node.Group, children: inner.nodes } });
      continue;
    }
    out.push(node);
  }
  return { nodes: out, removed };
}

/** Inserts `node` into `nodes` at the root (`parentGroupId === null`) or inside a group, at `index`. */
function insertNode(
  nodes: LibraryNode[],
  node: LibraryNode,
  parentGroupId: number | null,
  index: number
): LibraryNode[] {
  if (parentGroupId === null) {
    const out = [...nodes];
    out.splice(Math.max(0, Math.min(index, out.length)), 0, node);
    return out;
  }
  return nodes.map((n) => {
    if (!isGroup(n)) return n;
    if (n.Group.id === parentGroupId) {
      const children = [...n.Group.children];
      children.splice(Math.max(0, Math.min(index, children.length)), 0, node);
      return { Group: { ...n.Group, children } };
    }
    return { Group: { ...n.Group, children: insertNode(n.Group.children, node, parentGroupId, index) } };
  });
}

/** A group cannot become its own descendant; true when `groupId` nests inside itself via `into`. */
function wouldCycle(nodes: LibraryNode[], groupId: number, into: number): boolean {
  const group = findGroup(nodes, groupId);
  if (!group) return false;
  if (group.id === into) return true;
  return mediaIdsOfGroupIds(group.children).includes(into);
}

function findGroup(nodes: LibraryNode[], groupId: number): LibraryGroup | null {
  for (const node of nodes) {
    if (isGroup(node)) {
      if (node.Group.id === groupId) return node.Group;
      const found = findGroup(node.Group.children, groupId);
      if (found) return found;
    }
  }
  return null;
}

function mediaIdsOfGroupIds(nodes: LibraryNode[]): number[] {
  const out: number[] = [];
  for (const node of nodes) {
    if (isGroup(node)) {
      out.push(node.Group.id, ...mediaIdsOfGroupIds(node.Group.children));
    }
  }
  return out;
}

/**
 * Moves `key` (a media leaf or a group) to `index` inside `targetGroupId`
 * (`null` for the root). A no-op when the move would nest a group inside
 * itself or one of its own descendants.
 */
export function moveNode(
  nodes: LibraryNode[],
  key: string,
  targetGroupId: number | null,
  index: number
): LibraryNode[] {
  if (key.startsWith('group:') && targetGroupId !== null) {
    const groupId = Number(key.slice('group:'.length));
    if (wouldCycle(nodes, groupId, targetGroupId)) return nodes;
  }
  const { nodes: without, removed } = removeNode(nodes, key);
  if (!removed) return nodes;
  return insertNode(without, removed, targetGroupId, index);
}

/** Removes every media leaf naming an id in `ids`, pruning stale references after a delete. */
export function pruneMedia(nodes: LibraryNode[], ids: ReadonlySet<number>): LibraryNode[] {
  const out: LibraryNode[] = [];
  for (const node of nodes) {
    if (isGroup(node)) {
      out.push({ Group: { ...node.Group, children: pruneMedia(node.Group.children, ids) } });
      continue;
    }
    if (!ids.has(node.Media)) out.push(node);
  }
  return out;
}

/** A flat root listing every id in `mediaIds`, in order — the default tree shape. */
export function flatLibrary(mediaIds: number[]): LibraryNode[] {
  return mediaIds.map((id) => ({ Media: id }));
}
