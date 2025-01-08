export interface Project {
  // UUID of the project
  id: string;
  name: string;
  // UUIDs of files in the project
  files: string[];
  // Whether the project has been modified
  dirty: boolean;
}

export const projectsManager = (() => {
  let projects: Project[] = $state([
    {
      id: crypto.randomUUID(),
      name: 'Untitled Project 1',
      files: [],
      dirty: false,
    },
  ]);
  // Current project UUID
  let currentProject: string | null = $state(projects[0].id);
  let projectMap: Record<string, Project> = $derived(
    Object.fromEntries(projects.map((project: Project) => [project.id, project]))
  );

  return {
    get projects() {
      return projects;
    },
    set projects(projects: Project[]) {
      projects = projects;
    },
    createProject(name: string | null): Project {
      const project: Project = {
        id: crypto.randomUUID(),
        name:
          name ??
          `Untitled Project ${
            (projects
              .filter((project) => project.name.startsWith('Untitled Project'))
              .map((t) => parseInt(t.name.split(' ').at(-1) ?? '0'))
              .toSorted((a, b) => a - b)
              .at(-1) ?? 0) + 1
          }`,
        files: [],
        dirty: false,
      };
      projects.push(project);
      return project;
    },
    getProject(id: string): Project | null {
      return projectMap[id] || null;
    },
    deleteProject(id: string): void {
      projects = projects.filter((project) => project.id !== id);
    },
    get currentProject(): string | null {
      return currentProject;
    },
    set currentProject(project: string) {
      currentProject = project;
    },
  };
})();
