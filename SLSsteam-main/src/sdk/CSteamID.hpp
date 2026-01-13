#pragma once

#include "libmem/libmem.h"

#include <cstdint>

class CSteamId
{
public:
	uint32_t steamId; //0x4
	uint8_t universe; //0x5
	char _pad_0x5[0x3]; //0x8
};
