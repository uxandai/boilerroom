using System;
using System.Collections.Generic;

namespace SolusManifestApp.Models
{
    public class GreenLumaProfile
    {
        public string Id { get; set; } = Guid.NewGuid().ToString();
        public string Name { get; set; } = string.Empty;
        public DateTime CreatedAt { get; set; } = DateTime.UtcNow;
        public DateTime ModifiedAt { get; set; } = DateTime.UtcNow;
        public List<ProfileGame> Games { get; set; } = new();
    }

    public class ProfileGame
    {
        public string AppId { get; set; } = string.Empty;
        public string Name { get; set; } = string.Empty;
        public List<ProfileDepot> Depots { get; set; } = new();
        public List<ProfileDLC> DLCs { get; set; } = new();
        public DateTime AddedAt { get; set; } = DateTime.UtcNow;
    }

    public class ProfileDLC
    {
        public string AppId { get; set; } = string.Empty;
        public string Name { get; set; } = string.Empty;
        public List<ProfileDepot> Depots { get; set; } = new();
    }

    public class ProfileDepot
    {
        public string DepotId { get; set; } = string.Empty;
        public string Name { get; set; } = string.Empty;
        public string ManifestId { get; set; } = string.Empty;
        public string DecryptionKey { get; set; } = string.Empty;
    }

    public class ProfileData
    {
        public string Version { get; set; } = "1.0";
        public string ActiveProfileId { get; set; } = string.Empty;
        public List<GreenLumaProfile> Profiles { get; set; } = new();
    }

    public class ProfileExport
    {
        public string ExportVersion { get; set; } = "1.0";
        public DateTime ExportedAt { get; set; } = DateTime.UtcNow;
        public string Warning { get; set; } = "This export only contains games installed with profile tracking. Games installed before the profile feature will not be included.";
        public GreenLumaProfile Profile { get; set; } = new();
    }
}
