import { open } from '@tauri-apps/plugin-dialog';
import { readFile } from '@tauri-apps/plugin-fs';
import path from 'path-browserify';

export async function openFiles(accept: string): Promise<{ name: string; content: ArrayBuffer }[]> {
  if ('__TAURI_INTERNALS__' in window) {
    const file = await open({
      multiple: true,
      filters: [{ name: 'Audio', extensions: ['mp3', 'wav', 'ogg'] }],
    });
    console.log(file);
    return await Promise.all(file?.map(async (f) => ({ name: path.basename(f), content: await readFile(f) })) || []);
  } else {
    const fileInput = document.createElement('input');
    fileInput.type = 'file';
    fileInput.multiple = true;
    fileInput.accept = accept;
    fileInput.click();
    await new Promise((resolve) => {
      fileInput.addEventListener('change', () => {
        const files = fileInput.files;
        resolve([...(files || [])].map((file) => ({ name: file.name, content: file.arrayBuffer() })));
      });
    });
    return [];
  }
}
