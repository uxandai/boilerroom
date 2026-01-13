#include "cloudsync.hpp"

#include "../config.hpp"
#include "../globals.hpp"
#include "../log.hpp"

#include <cstdio>
#include <cstring>
#include <ctime>
#include <filesystem>
#include <fstream>
#include <thread>
#include <vector>
#include <mutex>

#include <curl/curl.h>

namespace CloudSync
{

static std::mutex g_cacheMutex;
static std::vector<std::string> g_fileList;  // Per-app file list cache

static std::string getCacheBaseDir()
{
	const char* configDir = getenv("XDG_DATA_HOME");
	if (configDir != nullptr)
	{
		return std::string(configDir) + "/SLSsteam/cloudsync";
	}

	const char* home = getenv("HOME");
	return std::string(home) + "/.local/share/SLSsteam/cloudsync";
}

std::string getCacheDir(uint32_t appId)
{
	return getCacheBaseDir() + "/" + std::to_string(appId);
}

bool shouldHandle(uint32_t appId)
{
	const auto& config = g_config.cloudSync.get();
	if (!config.enabled || config.webdavUrl.empty())
	{
		return false;
	}

	// If appIds is empty, handle all apps; otherwise check if appId is in the list
	if (config.appIds.empty())
	{
		return true;
	}

	return config.appIds.contains(appId);
}

static std::string getFilePath(uint32_t appId, const char* filename)
{
	return getCacheDir(appId) + "/" + filename;
}

static bool ensureDir(const std::string& dir)
{
	if (!std::filesystem::exists(dir))
	{
		return std::filesystem::create_directories(dir);
	}
	return true;
}

bool init()
{
	std::string baseDir = getCacheBaseDir();
	if (!ensureDir(baseDir))
	{
		g_pLog->warn("CloudSync: Failed to create cache directory: %s\n", baseDir.c_str());
		return false;
	}
	g_pLog->info("CloudSync: Cache directory: %s\n", baseDir.c_str());
	return true;
}

// --- File Operations ---

bool fileWrite(uint32_t appId, const char* filename, const void* data, int32_t dataSize)
{
	if (!shouldHandle(appId))
	{
		return false;
	}

	std::lock_guard<std::mutex> lock(g_cacheMutex);

	std::string dir = getCacheDir(appId);
	if (!ensureDir(dir))
	{
		g_pLog->warn("CloudSync: Failed to create app cache dir: %s\n", dir.c_str());
		return false;
	}

	std::string path = getFilePath(appId, filename);
	std::ofstream file(path, std::ios::binary);
	if (!file)
	{
		g_pLog->warn("CloudSync: Failed to write file: %s\n", path.c_str());
		return false;
	}

	file.write(static_cast<const char*>(data), dataSize);
	file.close();

	g_pLog->debug("CloudSync: Wrote %d bytes to %s\n", dataSize, filename);

	// Async upload to WebDAV
	std::thread([appId, fname = std::string(filename)]() {
		syncToWebDAV(appId, fname.c_str());
	}).detach();

	return true;
}

int32_t fileRead(uint32_t appId, const char* filename, void* buffer, int32_t bufferSize)
{
	if (!shouldHandle(appId))
	{
		return -1;
	}

	std::lock_guard<std::mutex> lock(g_cacheMutex);

	std::string path = getFilePath(appId, filename);
	std::ifstream file(path, std::ios::binary | std::ios::ate);
	if (!file)
	{
		// Try fetching from WebDAV first
		syncFromWebDAV(appId, filename);
		file.open(path, std::ios::binary | std::ios::ate);
		if (!file)
		{
			return -1;
		}
	}

	int32_t fileSize = static_cast<int32_t>(file.tellg());
	file.seekg(0, std::ios::beg);

	int32_t readSize = std::min(fileSize, bufferSize);
	file.read(static_cast<char*>(buffer), readSize);
	file.close();

	g_pLog->debug("CloudSync: Read %d bytes from %s\n", readSize, filename);
	return readSize;
}

bool fileDelete(uint32_t appId, const char* filename)
{
	if (!shouldHandle(appId))
	{
		return false;
	}

	std::lock_guard<std::mutex> lock(g_cacheMutex);

	std::string path = getFilePath(appId, filename);
	if (std::filesystem::exists(path))
	{
		std::filesystem::remove(path);
		g_pLog->debug("CloudSync: Deleted %s\n", filename);
		return true;
	}
	return false;
}

bool fileExists(uint32_t appId, const char* filename)
{
	if (!shouldHandle(appId))
	{
		return false;
	}

	std::string path = getFilePath(appId, filename);
	return std::filesystem::exists(path);
}

int32_t getFileSize(uint32_t appId, const char* filename)
{
	if (!shouldHandle(appId))
	{
		return -1;
	}

	std::string path = getFilePath(appId, filename);
	if (!std::filesystem::exists(path))
	{
		return -1;
	}

	return static_cast<int32_t>(std::filesystem::file_size(path));
}

int64_t getFileTimestamp(uint32_t appId, const char* filename)
{
	if (!shouldHandle(appId))
	{
		return 0;
	}

	std::string path = getFilePath(appId, filename);
	if (!std::filesystem::exists(path))
	{
		return 0;
	}

	auto ftime = std::filesystem::last_write_time(path);
	auto sctp = std::chrono::time_point_cast<std::chrono::seconds>(
		std::chrono::file_clock::to_sys(ftime));
	return sctp.time_since_epoch().count();
}

int32_t getFileCount(uint32_t appId)
{
	if (!shouldHandle(appId))
	{
		return 0;
	}

	std::string dir = getCacheDir(appId);
	if (!std::filesystem::exists(dir))
	{
		return 0;
	}

	int32_t count = 0;
	for (const auto& entry : std::filesystem::directory_iterator(dir))
	{
		if (entry.is_regular_file())
		{
			count++;
		}
	}
	return count;
}

const char* getFileNameAndSize(uint32_t appId, int index, int32_t* outSize)
{
	if (!shouldHandle(appId))
	{
		return nullptr;
	}

	static thread_local std::string s_filename;

	std::string dir = getCacheDir(appId);
	if (!std::filesystem::exists(dir))
	{
		return nullptr;
	}

	int i = 0;
	for (const auto& entry : std::filesystem::directory_iterator(dir))
	{
		if (entry.is_regular_file())
		{
			if (i == index)
			{
				s_filename = entry.path().filename().string();
				if (outSize)
				{
					*outSize = static_cast<int32_t>(entry.file_size());
				}
				return s_filename.c_str();
			}
			i++;
		}
	}
	return nullptr;
}

// --- WebDAV Operations ---

static size_t curlWriteCallback(void* contents, size_t size, size_t nmemb, void* userp)
{
	std::vector<char>* buffer = static_cast<std::vector<char>*>(userp);
	size_t totalSize = size * nmemb;
	buffer->insert(buffer->end(), static_cast<char*>(contents), static_cast<char*>(contents) + totalSize);
	return totalSize;
}

static size_t curlReadCallback(void* ptr, size_t size, size_t nmemb, void* userdata)
{
	std::ifstream* file = static_cast<std::ifstream*>(userdata);
	file->read(static_cast<char*>(ptr), size * nmemb);
	return static_cast<size_t>(file->gcount());
}

static std::string getWebDAVUrl(uint32_t appId, const char* filename)
{
	const auto& config = g_config.cloudSync.get();
	std::string url = config.webdavUrl;

	// Ensure trailing slash
	if (!url.empty() && url.back() != '/')
	{
		url += '/';
	}

	// Add app-specific path
	url += "SLSsteam/" + std::to_string(appId) + "/" + filename;
	return url;
}

void syncToWebDAV(uint32_t appId, const char* filename)
{
	const auto& config = g_config.cloudSync.get();
	if (!config.enabled || config.webdavUrl.empty())
	{
		return;
	}

	std::string localPath = getFilePath(appId, filename);
	if (!std::filesystem::exists(localPath))
	{
		return;
	}

	std::string url = getWebDAVUrl(appId, filename);

	CURL* curl = curl_easy_init();
	if (!curl)
	{
		g_pLog->warn("CloudSync: Failed to init curl for upload\n");
		return;
	}

	// First, ensure the directory exists on WebDAV (MKCOL)
	std::string parentUrl = config.webdavUrl;
	if (!parentUrl.empty() && parentUrl.back() != '/') parentUrl += '/';
	parentUrl += "SLSsteam/" + std::to_string(appId) + "/";

	curl_easy_setopt(curl, CURLOPT_URL, parentUrl.c_str());
	curl_easy_setopt(curl, CURLOPT_CUSTOMREQUEST, "MKCOL");
	if (!config.username.empty())
	{
		std::string userpwd = config.username + ":" + config.password;
		curl_easy_setopt(curl, CURLOPT_USERPWD, userpwd.c_str());
	}
	curl_easy_perform(curl);  // Ignore error - directory might already exist

	// Now upload the file (PUT)
	std::ifstream file(localPath, std::ios::binary | std::ios::ate);
	if (!file)
	{
		curl_easy_cleanup(curl);
		return;
	}

	size_t fileSize = static_cast<size_t>(file.tellg());
	file.seekg(0, std::ios::beg);

	curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
	curl_easy_setopt(curl, CURLOPT_UPLOAD, 1L);
	curl_easy_setopt(curl, CURLOPT_READFUNCTION, curlReadCallback);
	curl_easy_setopt(curl, CURLOPT_READDATA, &file);
	curl_easy_setopt(curl, CURLOPT_INFILESIZE_LARGE, static_cast<curl_off_t>(fileSize));
	curl_easy_setopt(curl, CURLOPT_CUSTOMREQUEST, NULL);  // Reset custom request

	if (!config.username.empty())
	{
		std::string userpwd = config.username + ":" + config.password;
		curl_easy_setopt(curl, CURLOPT_USERPWD, userpwd.c_str());
	}

	CURLcode res = curl_easy_perform(curl);
	if (res != CURLE_OK)
	{
		g_pLog->warn("CloudSync: WebDAV upload failed for %s: %s\n", filename, curl_easy_strerror(res));
	}
	else
	{
		g_pLog->debug("CloudSync: Uploaded %s to WebDAV\n", filename);
	}

	file.close();
	curl_easy_cleanup(curl);
}

void syncFromWebDAV(uint32_t appId, const char* filename)
{
	const auto& config = g_config.cloudSync.get();
	if (!config.enabled || config.webdavUrl.empty())
	{
		return;
	}

	std::string url = getWebDAVUrl(appId, filename);

	CURL* curl = curl_easy_init();
	if (!curl)
	{
		return;
	}

	std::vector<char> buffer;

	curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
	curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, curlWriteCallback);
	curl_easy_setopt(curl, CURLOPT_WRITEDATA, &buffer);

	if (!config.username.empty())
	{
		std::string userpwd = config.username + ":" + config.password;
		curl_easy_setopt(curl, CURLOPT_USERPWD, userpwd.c_str());
	}

	CURLcode res = curl_easy_perform(curl);
	long httpCode = 0;
	curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &httpCode);

	if (res == CURLE_OK && httpCode == 200 && !buffer.empty())
	{
		// Ensure directory exists
		std::string dir = getCacheDir(appId);
		ensureDir(dir);

		// Write to local cache
		std::string localPath = getFilePath(appId, filename);
		std::ofstream file(localPath, std::ios::binary);
		if (file)
		{
			file.write(buffer.data(), buffer.size());
			file.close();
			g_pLog->debug("CloudSync: Downloaded %s from WebDAV (%zu bytes)\n", filename, buffer.size());
		}
	}

	curl_easy_cleanup(curl);
}

void syncAllFromWebDAV(uint32_t appId)
{
	// This would require PROPFIND to list files on WebDAV
	// For now, leave as a stub - games will request files individually
	g_pLog->debug("CloudSync: syncAllFromWebDAV not yet implemented for appId %u\n", appId);
}

} // namespace CloudSync
