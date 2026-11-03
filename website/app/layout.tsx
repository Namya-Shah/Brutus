import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  metadataBase: new URL("https://brutus-agent-workspace.brownloach129.chatgpt.site"),
  title: "Brutus — Think in parallel. Build in Brutus.",
  description: "Your coding agents. Their own Git worktrees. One native workspace. Meet Brutus, a parallel agent IDE for macOS, Windows, and Linux. Join the early-access waitlist.",
  openGraph: {
    title: "Brutus — Think in parallel. Build in Brutus.",
    description: "Your favorite coding agents. Their own worktrees. One native workspace to keep it all in focus.",
    type: "website",
  },
  icons: {
    icon: "/favicon.svg",
    shortcut: "/favicon.svg",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className="antialiased">{children}</body>
    </html>
  );
}
