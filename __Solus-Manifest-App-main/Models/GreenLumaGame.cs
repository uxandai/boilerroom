using System;
using System.Collections.Generic;

namespace SolusManifestApp.Models
{
    public class GreenLumaGame
    {
        public string AppId { get; set; } = string.Empty;
        public string Name { get; set; } = string.Empty;
        public long SizeBytes { get; set; }
        public DateTime? InstallDate { get; set; }
        public DateTime? LastUpdated { get; set; }
        public List<string> AppListFilePaths { get; set; } = new();
        public List<string> DepotIds { get; set; } = new();
        public string AcfPath { get; set; } = string.Empty;
        public bool HasLuaFile { get; set; }
        public string? LuaFilePath { get; set; }
    }
}
