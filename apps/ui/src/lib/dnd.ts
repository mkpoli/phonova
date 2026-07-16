/** Drag-and-drop file collection, including folder traversal. */

interface FileSystemEntryLike {
  isFile: boolean;
  isDirectory: boolean;
  file?(onSuccess: (file: File) => void, onError: (error: unknown) => void): void;
  createReader?(): {
    readEntries(
      onSuccess: (entries: FileSystemEntryLike[]) => void,
      onError: (error: unknown) => void
    ): void;
  };
}

async function walkEntry(entry: FileSystemEntryLike, out: File[]): Promise<void> {
  if (entry.isFile && entry.file) {
    const file = await new Promise<File>((resolve, reject) => entry.file!(resolve, reject));
    out.push(file);
    return;
  }
  if (entry.isDirectory && entry.createReader) {
    const reader = entry.createReader();
    // readEntries hands back at most a page of children; keep reading until empty.
    for (;;) {
      const batch = await new Promise<FileSystemEntryLike[]>((resolve, reject) =>
        reader.readEntries(resolve, reject)
      );
      if (batch.length === 0) break;
      for (const child of batch) await walkEntry(child, out);
    }
  }
}

/**
 * Collects every file from a drop, walking into dropped folders.
 *
 * A folder drop exposes directory entries through `webkitGetAsEntry`; a plain
 * selection has none, so the drop's flat `files` list is used instead.
 */
export async function filesFromDataTransfer(data: DataTransfer): Promise<File[]> {
  const items = Array.from(data.items).filter((item) => item.kind === 'file');
  const entries = items
    .map((item) => (item.webkitGetAsEntry ? item.webkitGetAsEntry() : null))
    .filter((entry): entry is FileSystemEntry => entry !== null);
  if (entries.length === 0) return Array.from(data.files);
  const out: File[] = [];
  for (const entry of entries) await walkEntry(entry as unknown as FileSystemEntryLike, out);
  return out;
}
