#pragma once

#include <cstdint>
#include <string>

namespace CloudSync
{
	// Check if CloudSync is enabled and should handle this app
	bool shouldHandle(uint32_t appId);

	// Get the local cache directory for an app's cloud files
	std::string getCacheDir(uint32_t appId);

	// File operations that replace Steam Cloud
	bool fileWrite(uint32_t appId, const char* filename, const void* data, int32_t dataSize);
	int32_t fileRead(uint32_t appId, const char* filename, void* buffer, int32_t bufferSize);
	bool fileDelete(uint32_t appId, const char* filename);
	bool fileExists(uint32_t appId, const char* filename);
	int32_t getFileSize(uint32_t appId, const char* filename);
	int64_t getFileTimestamp(uint32_t appId, const char* filename);
	int32_t getFileCount(uint32_t appId);
	const char* getFileNameAndSize(uint32_t appId, int index, int32_t* outSize);

	// WebDAV sync operations (run asynchronously)
	void syncToWebDAV(uint32_t appId, const char* filename);
	void syncFromWebDAV(uint32_t appId, const char* filename);
	void syncAllFromWebDAV(uint32_t appId);

	// Initialize the cache directory
	bool init();
}
