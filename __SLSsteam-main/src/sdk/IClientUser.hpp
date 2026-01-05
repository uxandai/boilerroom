#pragma once

#include <cstdint>

class IClientUser
{
public:
	bool isLoggedOn();
	//Do not use, this ain't working the way CheckAppOwnership modifes things
	bool isSubscribed(uint32_t appId);
	uint32_t updateOwnershipInfo(uint32_t appId, bool staleOnly);
};

extern IClientUser* g_pClientUser;
