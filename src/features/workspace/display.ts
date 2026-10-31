/** MIME type of the bytes `load_image` returns for `path` (TIFF/ICO/GIF arrive as PNG). */
export function displayMime(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  if (["jpg", "jpeg", "jpe", "jfif"].includes(ext)) return "image/jpeg";
  if (["webp", "bmp", "avif"].includes(ext)) return `image/${ext}`;
  return "image/png";
}
