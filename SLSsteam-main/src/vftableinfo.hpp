#pragma once

namespace VFTIndexes
{
	namespace IClientEngine
	{
		constexpr int GetClientUser = 7;
	}

	namespace IClientApps
	{
		constexpr int RequestAppInfoUpdate = 7;
		constexpr int GetDLCCount = 8;
		constexpr int GetDLCDataByIndex = 9;
		constexpr int GetAppType = 10;
	}

	namespace IClientAppManager
	{
		constexpr int InstallApp = 0;
		constexpr int UninstallApp = 1;
		constexpr int LaunchApp = 2;
		constexpr int GetAppInstallState = 4;
		constexpr int IsAppDlcInstalled = 9;
		constexpr int BIsDlcEnabled = 11;
		constexpr int GetUpdateInfo = 20;
	}

	namespace IClientRemoteStorage
	{
		constexpr int FileWrite = 0;
		constexpr int FileRead = 1;
		constexpr int FileForget = 2;
		constexpr int FileDelete = 3;
		constexpr int FileExists = 6;
		constexpr int GetFileSize = 9;
		constexpr int GetFileTimestamp = 10;
		constexpr int GetFileCount = 12;
		constexpr int GetFileNameAndSize = 13;
		constexpr int IsCloudEnabledForApp = 24;
	}

	namespace IClientUser
	{
		constexpr int BLoggedOn = 4;
		constexpr int GetSteamID = 10;
	}

	namespace IClientUtils
	{
		constexpr int GetOfflineMode = 17;
		constexpr int GetAppId = 19;
	}
}
