#include "IClientUser.hpp"

#include "../hooks.hpp"
#include "../patterns.hpp"


bool IClientUser::isLoggedOn()
{
	return Hooks::IClientUser_BLoggedOn.tramp.fn(this);
}

bool IClientUser::isSubscribed(uint32_t appId)
{
	return Hooks::IClientUser_BIsSubscribedApp.tramp.fn(this, appId);
}

uint32_t IClientUser::updateOwnershipInfo(uint32_t appId, bool staleOnly)
{
	//Call hook to make ticket cache cooperate
	return Hooks::IClientUser_BUpdateAppOwnershipTicket.hookFn.fn(this, appId, staleOnly);
}

IClientUser* g_pClientUser;
