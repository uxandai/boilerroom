using System;

namespace SolusManifestApp.Models
{
    public class SteamGame
    {
        public string AppId { get; set; } = string.Empty;
        public string Name { get; set; } = string.Empty;
        public string InstallDir { get; set; } = string.Empty;
        public long SizeOnDisk { get; set; }
        public DateTime? LastUpdated { get; set; }
        public string LibraryPath { get; set; } = string.Empty;
        public string StateFlags { get; set; } = string.Empty;
        public bool IsFullyInstalled { get; set; }
        public string BuildId { get; set; } = string.Empty;

        public string SizeFormatted => FormatBytes(SizeOnDisk);

        private static string FormatBytes(long bytes)
        {
            string[] sizes = { "B", "KB", "MB", "GB", "TB" };
            double len = bytes;
            int order = 0;
            while (len >= 1024 && order < sizes.Length - 1)
            {
                order++;
                len /= 1024;
            }
            return $"{len:0.##} {sizes[order]}";
        }
    }
}
