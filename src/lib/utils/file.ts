import { open } from '@tauri-apps/plugin-dialog';
import { readFile } from '@tauri-apps/plugin-fs';
import path from 'path-browserify';
import { v4 as uuidv4 } from 'uuid';

export type File = {
  id: string;
  name: string;
  content: ArrayBuffer;
};

export async function openFiles(accept: string): Promise<File[]> {
  if ('__TAURI_INTERNALS__' in window) {
    const file = await open({
      multiple: true,
      filters: [{ name: 'Audio', extensions: ['mp3', 'wav', 'ogg'] }],
    });
    console.log(file);
    return await Promise.all(
      file?.map(async (f) => ({ id: uuidv4(), name: path.basename(f), content: await readFile(f) })) || []
    );
  } else {
    const fileInput = document.createElement('input');
    fileInput.type = 'file';
    fileInput.multiple = true;
    fileInput.accept = accept;
    fileInput.click();
    await new Promise((resolve) => {
      fileInput.addEventListener('change', () => {
        const files = fileInput.files;
        resolve([...(files || [])].map((file) => ({ id: uuidv4(), name: file.name, content: file.arrayBuffer() })));
      });
    });
    return [];
  }
}
