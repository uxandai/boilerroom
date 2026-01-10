import { memo } from "react";
import { Button } from "@/components/ui/button";
import { FolderOpen, Search, Upload, Trash2 } from "lucide-react";
import { formatSize } from "@/lib/utils";
import type { InstalledGame } from "@/lib/api";

interface LibraryGameItemProps {
  game: InstalledGame;
  artworkUrl?: string;
  connectionMode: "local" | "remote";
  onSearch: (game: InstalledGame) => void;
  onCopy?: (game: InstalledGame) => void;
  onUninstall: (game: InstalledGame) => void;
  onSelect: (game: InstalledGame) => void;
}

export const LibraryGameItem = memo(function LibraryGameItem({
  game,
  artworkUrl,
  connectionMode,
  onSearch,
  onCopy,
  onUninstall,
  onSelect
}: LibraryGameItemProps) {
  return (
    <div
      className="bg-[#171a21] border border-[#0a0a0a] p-3 flex items-center justify-between hover:bg-[#1b2838] transition-colors cursor-pointer"
      onClick={() => onSelect(game)}
    >
      <div className="flex items-center gap-4">
        {artworkUrl || game.header_image ? (
          <img
            src={artworkUrl || game.header_image}
            alt={game.name}
            className="w-24 h-9 object-cover rounded"
          />
        ) : (
          <div className="w-24 h-9 bg-[#2a475e] rounded flex items-center justify-center">
            <FolderOpen className="w-4 h-4 text-muted-foreground" />
          </div>
        )}
        <div>
          <p className="font-medium text-white">{game.name}</p>
          <p className="text-xs text-muted-foreground">
            AppID: {game.app_id} • {formatSize(game.size_bytes)}
          </p>
        </div>
      </div>
      <div className="flex items-center gap-2">
        <Button
          variant="ghost"
          size="sm"
          className="text-[#67c1f5] hover:text-[#8ed0f8] hover:bg-[#2a475e]"
          title="Search for updates"
          onClick={(e) => {
            e.stopPropagation();
            onSearch(game);
          }}
        >
          <Search className="w-4 h-4" />
        </Button>
        {/* Copy to Remote - only in local mode */}
        {connectionMode === "local" && onCopy && (
          <Button
            variant="ghost"
            size="sm"
            className="text-green-400 hover:text-green-300 hover:bg-green-900/20"
            title="Copy to Steam Deck"
            onClick={(e) => {
              e.stopPropagation();
              onCopy(game);
            }}
          >
            <Upload className="w-4 h-4" />
          </Button>
        )}
        <Button
          variant="ghost"
          size="sm"
          className="text-red-400 hover:text-red-300 hover:bg-red-900/20"
          title="Uninstall"
          onClick={(e) => {
            e.stopPropagation();
            onUninstall(game);
          }}
        >
          <Trash2 className="w-4 h-4" />
        </Button>
      </div>
    </div>
  );
});
