import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "PolyInsight Engine",
  description: "Prediction market data terminal",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className="dark">
      <body className="min-h-screen bg-[#0a0a0a] text-slate-200 antialiased">
        {children}
      </body>
    </html>
  );
}
