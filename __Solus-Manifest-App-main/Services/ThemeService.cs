using SolusManifestApp.Models;
using System;
using System.Linq;
using System.Windows;

namespace SolusManifestApp.Services
{
    public class ThemeService
    {
        public void ApplyTheme(AppTheme theme)
        {
            var themeFile = GetThemeFileName(theme);
            var themeUri = new Uri($"pack://application:,,,/Resources/Themes/{themeFile}", UriKind.Absolute);

            Application.Current.Dispatcher.Invoke(() =>
            {
                // Store other dictionaries (like SteamTheme.xaml)
                var otherDictionaries = Application.Current.Resources.MergedDictionaries
                    .Skip(1)
                    .ToList();

                // Clear all and reload with new theme first
                Application.Current.Resources.MergedDictionaries.Clear();

                // Add new theme first
                var newTheme = new ResourceDictionary { Source = themeUri };
                Application.Current.Resources.MergedDictionaries.Add(newTheme);

                // Re-add other dictionaries
                foreach (var dict in otherDictionaries)
                {
                    Application.Current.Resources.MergedDictionaries.Add(dict);
                }
            });
        }

        private string GetThemeFileName(AppTheme theme)
        {
            return theme switch
            {
                AppTheme.Default => "DefaultTheme.xaml",
                AppTheme.Dark => "DarkTheme.xaml",
                AppTheme.Light => "LightTheme.xaml",
                AppTheme.Cherry => "CherryTheme.xaml",
                AppTheme.Sunset => "SunsetTheme.xaml",
                AppTheme.Forest => "ForestTheme.xaml",
                AppTheme.Grape => "GrapeTheme.xaml",
                AppTheme.Cyberpunk => "CyberpunkTheme.xaml",
                _ => "DefaultTheme.xaml"
            };
        }
    }
}
