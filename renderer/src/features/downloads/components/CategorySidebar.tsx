import { Button } from "@/components/ui/button";
import { DOWNLOAD_CATEGORIES } from "@/types/ipc";
import {
  type CategoryFilter,
  useDownloadsStore,
} from "../state/downloads-store";

const ITEMS: ReadonlyArray<{ value: CategoryFilter; label: string }> = [
  { value: "all", label: "All downloads" },
  ...DOWNLOAD_CATEGORIES.map((category) => ({
    value: category,
    label: category,
  })),
];

export function CategorySidebar() {
  const category = useDownloadsStore((state) => state.category);
  const setCategory = useDownloadsStore((state) => state.setCategory);

  return (
    <aside className="w-40 shrink-0 border-r border-border bg-card">
      <div className="border-b border-border p-2 text-sm font-semibold">
        Categories
      </div>
      <nav className="space-y-1 p-2" aria-label="Categories">
        {ITEMS.map((item) => (
          <Button
            key={item.value}
            type="button"
            variant={category === item.value ? "secondary" : "ghost"}
            onClick={() => setCategory(item.value)}
            aria-current={category === item.value ? "page" : undefined}
            className="w-full justify-start"
          >
            {item.label}
          </Button>
        ))}
      </nav>
    </aside>
  );
}
