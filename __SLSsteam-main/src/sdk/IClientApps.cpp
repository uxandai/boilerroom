#include "IClientApps.hpp"

#include "../memhlp.hpp"
#include "../vftableinfo.hpp"

#include <cstdint>

EAppType IClientApps::getAppType(uint32_t appId)
{
	return MemHlp::callVFunc<EAppType(*)(void*, uint32_t)>(VFTIndexes::IClientApps::GetAppType, this, appId);
}

IClientApps* g_pClientApps;
