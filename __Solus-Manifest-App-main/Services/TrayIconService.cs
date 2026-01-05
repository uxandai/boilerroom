using System;
using System.Windows;
using System.Windows.Forms;
using System.IO;
using System.Drawing;
using System.Reflection;
using System.Diagnostics;
using SolusManifestApp.ViewModels;
using SolusManifestApp.Models;

namespace SolusManifestApp.Services
{
    public class TrayIconService : IDisposable
    {
        private NotifyIcon? _notifyIcon;
        private readonly Window _mainWindow;
        private readonly SettingsService _settingsService;
        private readonly RecentGamesService _recentGamesService;
        private readonly SteamService _steamService;
        private readonly MainViewModel _mainViewModel;
        private readonly ThemeService _themeService;

        public TrayIconService(
            Window mainWindow,
            SettingsService settingsService,
            RecentGamesService recentGamesService,
            SteamService steamService,
            MainViewModel mainViewModel,
            ThemeService themeService)
        {
            _mainWindow = mainWindow;
            _settingsService = settingsService;
            _recentGamesService = recentGamesService;
            _steamService = steamService;
            _mainViewModel = mainViewModel;
            _themeService = themeService;
        }

        public void Initialize()
        {
            var settings = _settingsService.LoadSettings();

            _notifyIcon = new NotifyIcon
            {
                Text = "Solus Manifest App",
                Visible = settings.AlwaysShowTrayIcon  // Controlled by settings
            };

            // Load icon from embedded resources first, then try file path
            try
            {
                var assembly = Assembly.GetExecutingAssembly();
                var resourceName = "SolusManifestApp.icon.ico";

                using (var stream = assembly.GetManifestResourceStream(resourceName))
                {
                    if (stream != null)
                    {
                        _notifyIcon.Icon = new Icon(stream);
                    }
                    else
                    {
                        // Try loading from file path as fallback
                        var iconPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "icon.ico");
                        if (File.Exists(iconPath))
                        {
                            _notifyIcon.Icon = new Icon(iconPath);
                        }
                        else
                        {
                            _notifyIcon.Icon = SystemIcons.Application;
                        }
                    }
                }
            }
            catch
            {
                // Use default icon if loading fails
                _notifyIcon.Icon = SystemIcons.Application;
            }

            // Create context menu and populate it initially
            var contextMenu = new ContextMenuStrip();
            RebuildContextMenu(contextMenu);  // Build menu immediately
            contextMenu.Opening += (s, e) => RebuildContextMenu(contextMenu);  // Rebuild on each open

            _notifyIcon.ContextMenuStrip = contextMenu;
            _notifyIcon.DoubleClick += (s, e) => ShowWindow();
        }

        private void RebuildContextMenu(ContextMenuStrip contextMenu)
        {
            contextMenu.Items.Clear();

            // Apply theme colors
            var settings = _settingsService.LoadSettings();
            var isDark = settings.Theme == AppTheme.Dark;

            if (isDark)
            {
                contextMenu.BackColor = Color.FromArgb(32, 32, 32);
                contextMenu.ForeColor = Color.White;
                contextMenu.Renderer = new DarkMenuRenderer();
            }
            else
            {
                contextMenu.BackColor = Color.White;
                contextMenu.ForeColor = Color.Black;
                contextMenu.Renderer = new ToolStripProfessionalRenderer();
            }

            // Recent section
            var recentGames = _recentGamesService.GetRecentGames(5);
            if (recentGames.Count > 0)
            {
                var recentHeader = new ToolStripMenuItem("Recent")
                {
                    Enabled = false,
                    Font = new Font(contextMenu.Font, System.Drawing.FontStyle.Bold)
                };
                contextMenu.Items.Add(recentHeader);

                foreach (var game in recentGames)
                {
                    var gameItem = new ToolStripMenuItem(game.Name);

                    // Load icon if available
                    if (!string.IsNullOrEmpty(game.IconPath) && File.Exists(game.IconPath))
                    {
                        try
                        {
                            using (var img = Image.FromFile(game.IconPath))
                            {
                                var bitmap = new Bitmap(img, new System.Drawing.Size(16, 16));
                                gameItem.Image = bitmap;
                            }
                        }
                        catch { /* Ignore icon loading errors */ }
                    }

                    var localAppId = game.AppId;
                    var localPath = game.LocalPath;
                    gameItem.Click += (s, e) => OpenGameLocation(localAppId, localPath);
                    contextMenu.Items.Add(gameItem);
                }

                contextMenu.Items.Add(new ToolStripSeparator());
            }

            // Tasks section
            var tasksHeader = new ToolStripMenuItem("Tasks")
            {
                Enabled = false,
                Font = new Font(contextMenu.Font, System.Drawing.FontStyle.Bold)
            };
            contextMenu.Items.Add(tasksHeader);

            var storeItem = new ToolStripMenuItem("Store");
            storeItem.Click += (s, e) => NavigateToPage("Store");
            contextMenu.Items.Add(storeItem);

            var libraryItem = new ToolStripMenuItem("Library");
            libraryItem.Click += (s, e) => NavigateToPage("Library");
            contextMenu.Items.Add(libraryItem);

            var downloadsItem = new ToolStripMenuItem("Downloads");
            downloadsItem.Click += (s, e) => NavigateToPage("Downloads");
            contextMenu.Items.Add(downloadsItem);

            var settingsItem = new ToolStripMenuItem("Settings");
            settingsItem.Click += (s, e) => NavigateToPage("Settings");
            contextMenu.Items.Add(settingsItem);

            var toolsItem = new ToolStripMenuItem("Tools");
            toolsItem.Click += (s, e) => NavigateToPage("Tools");
            contextMenu.Items.Add(toolsItem);

            contextMenu.Items.Add(new ToolStripSeparator());

            // Standard items
            var showItem = new ToolStripMenuItem("Show App");
            showItem.Click += (s, e) => ShowWindow();
            contextMenu.Items.Add(showItem);

            var exitItem = new ToolStripMenuItem("Exit");
            exitItem.Click += (s, e) => ExitApplication();
            contextMenu.Items.Add(exitItem);
        }

        private void NavigateToPage(string pageName)
        {
            ShowWindow();
            _mainViewModel.NavigateTo(pageName);
        }

        private void OpenGameLocation(string appId, string localPath)
        {
            try
            {
                // Try to open the game's local path
                if (!string.IsNullOrEmpty(localPath) && (File.Exists(localPath) || Directory.Exists(localPath)))
                {
                    Process.Start(new ProcessStartInfo
                    {
                        FileName = "explorer.exe",
                        Arguments = File.Exists(localPath) ? $"/select,\"{localPath}\"" : $"\"{localPath}\"",
                        UseShellExecute = true
                    });
                }
                else
                {
                    // Fallback: Try to find lua file in stplug-in
                    var stpluginPath = _steamService.GetStPluginPath();
                    if (!string.IsNullOrEmpty(stpluginPath))
                    {
                        var luaFile = Path.Combine(stpluginPath, $"{appId}.lua");
                        if (File.Exists(luaFile))
                        {
                            Process.Start(new ProcessStartInfo
                            {
                                FileName = "explorer.exe",
                                Arguments = $"/select,\"{luaFile}\"",
                                UseShellExecute = true
                            });
                        }
                    }
                }
            }
            catch
            {
                // Silent fail - just don't open anything
            }
        }

        public void ShowInTray()
        {
            if (_notifyIcon != null)
            {
                // Ensure icon is set before showing
                if (_notifyIcon.Icon == null)
                {
                    _notifyIcon.Icon = SystemIcons.Application;
                }

                _mainWindow.Hide();
                _notifyIcon.Visible = true;
            }
        }

        public void HideFromTray()
        {
            if (_notifyIcon != null)
            {
                var settings = _settingsService.LoadSettings();
                // Only hide if not set to always show
                if (!settings.AlwaysShowTrayIcon)
                {
                    _notifyIcon.Visible = false;
                }
            }
        }

        private void ShowWindow()
        {
            _mainWindow.Show();
            _mainWindow.WindowState = WindowState.Normal;
            _mainWindow.Activate();
            HideFromTray();
        }

        private void ExitApplication()
        {
            _notifyIcon?.Dispose();
            System.Windows.Application.Current.Shutdown();
        }

        public void Dispose()
        {
            _notifyIcon?.Dispose();
        }
    }

    // Custom renderer for dark theme context menu
    public class DarkMenuRenderer : ToolStripProfessionalRenderer
    {
        public DarkMenuRenderer() : base(new DarkColorTable())
        {
        }

        protected override void OnRenderArrow(ToolStripArrowRenderEventArgs e)
        {
            e.Graphics.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
            var r = new Rectangle(e.ArrowRectangle.Location, e.ArrowRectangle.Size);
            r.Inflate(-2, -6);
            e.Graphics.DrawLines(Pens.White, new System.Drawing.Point[]{
                new System.Drawing.Point(r.Left, r.Top),
                new System.Drawing.Point(r.Right, r.Top + r.Height / 2),
                new System.Drawing.Point(r.Left, r.Top + r.Height)
            });
        }

        protected override void OnRenderItemCheck(ToolStripItemImageRenderEventArgs e)
        {
            e.Graphics.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
            var r = new Rectangle(e.ImageRectangle.Location, e.ImageRectangle.Size);
            r.Inflate(-4, -6);
            e.Graphics.DrawLines(Pens.White, new System.Drawing.Point[]{
                new System.Drawing.Point(r.Left, r.Bottom - r.Height / 2),
                new System.Drawing.Point(r.Left + r.Width / 3, r.Bottom),
                new System.Drawing.Point(r.Right, r.Top)
            });
        }
    }

    // Custom color table for dark theme
    public class DarkColorTable : ProfessionalColorTable
    {
        public override Color MenuItemSelected => Color.FromArgb(62, 62, 64);
        public override Color MenuItemSelectedGradientBegin => Color.FromArgb(62, 62, 64);
        public override Color MenuItemSelectedGradientEnd => Color.FromArgb(62, 62, 64);
        public override Color MenuItemBorder => Color.FromArgb(51, 51, 55);
        public override Color MenuBorder => Color.FromArgb(51, 51, 55);
        public override Color MenuItemPressedGradientBegin => Color.FromArgb(51, 51, 55);
        public override Color MenuItemPressedGradientEnd => Color.FromArgb(51, 51, 55);
        public override Color ImageMarginGradientBegin => Color.FromArgb(37, 37, 38);
        public override Color ImageMarginGradientMiddle => Color.FromArgb(37, 37, 38);
        public override Color ImageMarginGradientEnd => Color.FromArgb(37, 37, 38);
        public override Color ToolStripDropDownBackground => Color.FromArgb(32, 32, 32);
    }
}
