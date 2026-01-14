import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/**
 * Format bytes to human-readable string (B, KB, MB, GB)
 */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

/**
 * Sort Steam libraries - internal storage first, then SD cards
 */
export function sortSteamLibraries(libraries: string[]): string[] {
  return [...libraries].sort((a, b) => {
    const aIsInternal = a.includes('.steam') || (!a.includes('mmcblk') && !a.includes('media'));
    const bIsInternal = b.includes('.steam') || (!b.includes('mmcblk') && !b.includes('media'));
    if (aIsInternal && !bIsInternal) return -1;
    if (!aIsInternal && bIsInternal) return 1;
    return 0;
  });
}
