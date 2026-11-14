import { useState, type ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";
import { useThemeStore, useThemeSync } from "@/stores/theme-store";

function ThemeBridge() {
  useThemeSync();
  return null;
}

/**
 * Application-wide providers. Mounted once by `main.tsx`: one query cache,
 * one theme owner, one toast host.
 */
export function Providers({ children }: { children: ReactNode }) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: { retry: 1, refetchOnWindowFocus: false },
        },
      }),
  );
  const effective = useThemeStore((state) => state.effective);

  return (
    <QueryClientProvider client={queryClient}>
      <ThemeBridge />
      {children}
      <Toaster theme={effective} richColors closeButton />
    </QueryClientProvider>
  );
}
