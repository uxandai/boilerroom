# Solus Manifest App - v2025.11.24.03

## 🎉 GBE Token Generator UX Improvements

### ✨ What's New

**🔔 Smart API Key Notifications**
- Now shows a helpful notification when you access the GBE Token Generator tab
- Reminds you to set up your Steam Web API key if you haven't already
- Only appears when clicking the GBE Token Generator tab (not on app startup or Tools tab)
- Uses the app's custom styled message box for consistency
- Shows only once per session - won't annoy you repeatedly!

**⚙️ Better Settings Organization**
- GBE Token Generator settings moved to **Settings > Advanced Tools** as its own sub-tab
- Now alongside SteamAuth Pro and Depot Key Extractor
- All advanced tools grouped together in one place

---

### 📥 Download

**Latest Release**: [v2025.11.24.03](https://github.com/MorrenusGames/Solus-Manifest-App/releases/tag/v2025.11.24.03)

---

# Previous Release - v2025.11.24.01

## 🎉 Major Release - UX Improvements & GBE Token Generator

### ✨ What's New

**🔑 GBE Token Generator**
We've integrated a complete GBE (Goldberg Emulator) Token Generator directly into the app!
- Find it under: **Tools > Denuvo > GBE Token Generator**
- Generate tokens with automatic Steam API integration
- Auto-fetches achievements, depots, DLCs, and language data
- Real-time logging so you can see what's happening
- Creates complete token packages ready to use
- **Credits**: Special thanks to Detanup01, NotAndreh, and Oureverday for the original tool!

### 🔧 Quality of Life Improvements

**🔐 Store API Key**
- API key validation popup now only appears when you click the Store tab
- No more annoying popup on every app launch if you don't use the Store!

**🎮 GreenLuma Updates**
- Renamed "Stealth (User32)" to just "Stealth" (following the 006 DLL name change)
- Uninstalling games now uses Steam's built-in `steam://uninstall/` protocol
- Much cleaner and lets Steam handle the cleanup properly

**📜 Store Navigation**
- Store listing now automatically scrolls to the top when you change pages
- No more being stuck at the bottom when navigating!

### 🐛 Big Bug Fix - Update Notifications

**FINALLY FIXED THE NOTIFICATION SPAM!**
If you have slow internet and tried to update, you know what I'm talking about... the app would spam you with hundreds of notifications. This is now FIXED:
- Shows only **ONE** notification when downloading: "Downloading the latest version... This may take a few minutes."
- No more notification on every 8KB downloaded
- Auto-update also won't spam you with "app is up to date" messages anymore
- Only notifies when updates are **actually available**

---

### 📥 Download

**Previous Release**: [v2025.11.24.01](https://github.com/MorrenusGames/Solus-Manifest-App/releases/tag/v2025.11.24.01)

---

### 📝 Full Changelog

See the complete changelog here: https://github.com/MorrenusGames/Solus-Manifest-App/blob/main/CHANGELOG.md
