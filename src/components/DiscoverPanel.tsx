import { useEffect, useState } from "react";
import { useAppStore } from "@/store/useAppStore";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Search, Heart, Flame, Sparkles, AlertCircle, Loader2 } from "lucide-react";
import { fetchWishlist, fetchFeaturedCategories } from "@/lib/api";

interface SimpleGame {
    appId: string;
    name: string;
    image: string;
    price?: string;
}

// Helper to separate SteamID64 from [U:1:...] format (SteamID3)
function getSteamId64(input: string): string {
    const clean = input.trim();
    // Check for [U:1:123456] format
    const match = clean.match(/^\[U:1:(\d+)\]$/);
    if (match && match[1]) {
        try {
            // SteamID64 = AccountID + 76561197960265728
            const accountId = BigInt(match[1]);
            const base = 76561197960265728n;
            return (accountId + base).toString();
        } catch (e) {
            return clean;
        }
    }
    return clean;
}

export function DiscoverPanel() {
    const { settings, setActiveTab, setSearchQuery, setTriggerSearch, settingsLoaded } = useAppStore();
    const [wishlist, setWishlist] = useState<SimpleGame[]>([]);
    const [topSellers, setTopSellers] = useState<SimpleGame[]>([]);
    const [newReleases, setNewReleases] = useState<SimpleGame[]>([]);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        if (settingsLoaded) {
            loadData();
        }
    }, [settings.steamUserId, settingsLoaded]);

    const loadData = async () => {
        setLoading(true);
        setError(null);
        try {
            // 1. Fetch Featured (Top Sellers, New Releases) - Works without ID
            try {
                const featuredJson = await fetchFeaturedCategories();
                const featuredData = JSON.parse(featuredJson);

                // Parse Top Sellers
                if (featuredData.top_sellers?.items) {
                    const parsedTop = featuredData.top_sellers.items.map((item: any) => ({
                        appId: item.id.toString(),
                        name: item.name,
                        image: item.header_image || item.large_capsule_image,
                        price: item.final_price ? (item.final_price / 100).toFixed(2) + " " + item.currency : "Free"
                    }));
                    setTopSellers(parsedTop);
                }

                // Parse New Releases
                if (featuredData.new_releases?.items) {
                    const parsedNew = featuredData.new_releases.items.map((item: any) => ({
                        appId: item.id.toString(),
                        name: item.name,
                        image: item.header_image || item.large_capsule_image,
                        price: item.final_price ? (item.final_price / 100).toFixed(2) + " " + item.currency : "Free"
                    }));
                    setNewReleases(parsedNew);
                }
            } catch (e) {
                console.error("Failed to load featured:", e);
                // Don't fail completely if featured fails, still try wishlist
            }

            // 2. Fetch Wishlist (if ID present)
            if (settings.steamUserId) {
                try {
                    const targetId = getSteamId64(settings.steamUserId);
                    const wishlistJson = await fetchWishlist(targetId, settings.steamApiKey);
                    console.log("[Discover] Raw Wishlist Response for", targetId, ":", wishlistJson);

                    // Check if response is HTML (starts with <)
                    if (wishlistJson.trim().startsWith("<")) {
                        console.error("[Discover] Received HTML instead of JSON. Profile likely private.");
                        throw new Error("Received HTML response. Your Steam Game Details might be private.");
                    }

                    const wishlistData = JSON.parse(wishlistJson);
                    let games: SimpleGame[] = [];

                    // 1. Handle IWishlistService format: { response: { items: [ { appid: 123, ... } ] } }
                    if (wishlistData.response && wishlistData.response.items) {
                        games = wishlistData.response.items.map((item: any) => ({
                            appId: item.appid.toString(),
                            name: `App ${item.appid}`, // Web API doesn't give names :(
                            image: `https://cdn.cloudflare.steamstatic.com/steam/apps/${item.appid}/header.jpg`,
                            price: "Check Store"
                        }));
                    }
                    // 2. Handle Storefront format (Old/Scraper): { "123": { name: "...", capsule: "..." } } or Array
                    else if (Array.isArray(wishlistData)) {
                        // Empty array or array of items (if format changed)
                        games = wishlistData.map((item: any) => ({
                            appId: item.appid ? item.appid.toString() : "0",
                            name: item.name || `App ${item.appid}`,
                            image: item.capsule || `https://cdn.cloudflare.steamstatic.com/steam/apps/${item.appid}/header.jpg`,
                            price: item.price
                        }));
                    } else if (typeof wishlistData === 'object') {
                        // Object format "appid": { ... }
                        games = Object.entries(wishlistData).map(([appId, data]: [string, any]) => ({
                            appId: appId,
                            name: data.name,
                            image: data.capsule, // 'capsule' is the landscape image usually
                            price: data.subs?.[0]?.price ? (data.subs[0].price / 100).toFixed(2) : undefined
                        }));
                    }

                    // Sort by priority if available, otherwise just name or random
                    setWishlist(games);
                } catch (e) {
                    console.error("Failed to load wishlist:", e);
                    let msg = "Failed to load wishlist";
                    if (e instanceof Error) {
                        if (e.message.includes("JSON Parse error") || e.message.includes("SyntaxError")) {
                            msg = "Failed to parse Steam response (Profile might be Private)";
                        } else {
                            msg = e.message;
                        }
                    }
                    setError(msg);
                }
            }

        } catch (e) {
            setError("Failed to load discover data");
        } finally {
            setLoading(false);
        }
    };

    const handleSearch = (appId: string) => {
        // Determine if we search by AppID or Name. 
        // AppID search is precise but searchBundles might expect name sometimes? 
        // Actually the new searchBundles supports AppID if implemented well, 
        // but usually user searches by name.
        // However, the USER REQUEST said: "klikne w lupke przy grze to wkleja steamappid do download search"
        // So we paste APPID.
        setSearchQuery(appId);
        setActiveTab("search");
        setTriggerSearch(true);
    };

    const GameGrid = ({ games }: { games: SimpleGame[] }) => (
        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
            {games.map((game) => (
                <div
                    key={game.appId}
                    className="group relative bg-[#171a21] border border-[#0a0a0a] hover:border-[#67c1f5] transition-colors rounded overflow-hidden"
                >
                    <div className="aspect-video w-full overflow-hidden bg-[#0a0a0a] relative">
                        <img
                            src={game.image}
                            alt={game.name}
                            loading="lazy"
                            className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
                            onError={(e) => {
                                const target = e.target as HTMLImageElement;
                                target.src = "https://steamdb.info/static/img/default.jpg"; // Fallback
                            }}
                        />
                        {/* Hover Overlay */}
                        <div className="absolute inset-0 bg-black/60 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center gap-2">
                            <Button
                                variant="secondary"
                                size="sm"
                                className="gap-2"
                                onClick={() => handleSearch(game.appId)}
                            >
                                <Search className="w-4 h-4" />
                                Download
                            </Button>
                        </div>
                    </div>
                    <div className="p-3">
                        <div className="font-medium text-white truncate" title={game.name}>{game.name}</div>
                        {game.price && (
                            <div className="text-xs text-gray-400 mt-1">{game.price}</div>
                        )}
                    </div>
                </div>
            ))}
        </div>
    );

    return (
        <div className="space-y-6 h-full flex flex-col">
            <div className="flex items-center justify-between">
                <h2 className="text-2xl font-bold flex items-center gap-2 text-white">
                    <Sparkles className="w-6 h-6 text-[#67c1f5]" />
                    Discover
                </h2>
                {settingsLoaded && !settings.steamUserId && (
                    <div className="flex items-center gap-2 text-sm text-yellow-400 bg-yellow-400/10 px-3 py-1 rounded border border-yellow-400/20">
                        <AlertCircle className="w-4 h-4" />
                        <span>Add Steam ID in Settings to see your Wishlist</span>
                    </div>
                )}
            </div>

            <Tabs defaultValue={settings.steamUserId ? "wishlist" : "featured"} className="w-full flex-1 flex flex-col">
                <TabsList className="bg-[#1b2838] border-b border-[#2a475e] w-full justify-start rounded-none h-auto p-0">
                    {settings.steamUserId && (
                        <TabsTrigger
                            value="wishlist"
                            className="data-[state=active]:bg-[#2a475e] data-[state=active]:text-white rounded-none px-6 py-3 h-auto gap-2 border-b-2 border-transparent data-[state=active]:border-[#67c1f5]"
                        >
                            <Heart className="w-4 h-4 text-red-500" />
                            My Wishlist
                            <span className="bg-[#0a0a0a] text-xs px-2 py-0.5 rounded-full ml-1 text-gray-400">
                                {wishlist.length}
                            </span>
                        </TabsTrigger>
                    )}
                    <TabsTrigger
                        value="featured"
                        className="data-[state=active]:bg-[#2a475e] data-[state=active]:text-white rounded-none px-6 py-3 h-auto gap-2 border-b-2 border-transparent data-[state=active]:border-[#67c1f5]"
                    >
                        <Flame className="w-4 h-4 text-orange-500" />
                        Top Sellers
                    </TabsTrigger>
                    <TabsTrigger
                        value="new"
                        className="data-[state=active]:bg-[#2a475e] data-[state=active]:text-white rounded-none px-6 py-3 h-auto gap-2 border-b-2 border-transparent data-[state=active]:border-[#67c1f5]"
                    >
                        <Sparkles className="w-4 h-4 text-blue-400" />
                        New Releases
                    </TabsTrigger>
                </TabsList>

                <ScrollArea className="flex-1 bg-[#1b2838] border-x border-b border-[#2a475e] p-6 h-0 min-h-[500px]">
                    {loading && (
                        <div className="flex flex-col items-center justify-center py-20 gap-4 text-muted-foreground">
                            <Loader2 className="w-10 h-10 animate-spin text-[#67c1f5]" />
                            <p>Loading Steam Store data...</p>
                        </div>
                    )}

                    {error && (
                        <div className="flex flex-col items-center justify-center py-20 gap-4 text-red-400">
                            <AlertCircle className="w-10 h-10" />
                            <p>{error}</p>
                            <Button variant="outline" onClick={() => loadData()}>Retry</Button>
                        </div>
                    )}

                    {!loading && !error && (
                        <>
                            <TabsContent value="wishlist" className="mt-0 space-y-4">
                                {(!settings.steamUserId || wishlist.length === 0) ? (
                                    <div className="text-center py-20 flex flex-col items-center">
                                        <p className="text-muted-foreground mb-4">No wishlist games found.</p>
                                        {!settings.steamUserId && (
                                            <Button
                                                variant="outline"
                                                onClick={() => setActiveTab("settings")}
                                            >
                                                Go to Settings
                                            </Button>
                                        )}
                                    </div>
                                ) : (
                                    <GameGrid games={wishlist} />
                                )}
                            </TabsContent>

                            <TabsContent value="featured" className="mt-0">
                                <GameGrid games={topSellers} />
                            </TabsContent>

                            <TabsContent value="new" className="mt-0">
                                <GameGrid games={newReleases} />
                            </TabsContent>
                        </>
                    )}
                </ScrollArea>
            </Tabs>
        </div>
    );
}
