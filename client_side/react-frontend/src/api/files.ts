import { cncAxios } from "./cncAxios";
import { PromiseResult, ReloadablePromiseResult, useGet } from "./generic";

export interface DirectoryItem {
  name: string,
  is_file: boolean,
}

// Directory should be something like "" or "x" or "x/" or "a/b" or "a/b/"
export function useDirectoryListing(directory: string): ReloadablePromiseResult<DirectoryItem[]> {
  return useGet("/job/list/" + directory);
}
export async function executeFile(path: string): Promise<void> {
  await cncAxios.post("/job/run_file", { path });
}
export async function deleteFile(path: string): Promise<void> {
  await cncAxios.delete("/job/delete_file", { data: { path, is_directory: false }, headers: {
        'Content-Type': 'application/json',
    }
 });
}
export async function uploadFile(path: string, file: Blob): Promise<void> {
  const form = new FormData();
  form.append("filename", path);
  form.append("file", file);
  await cncAxios.post("/job/upload_file", form);
}
export async function createDirectory(path: string): Promise<void> {
  await cncAxios.post("/job/create_directory", { directory: path });
}