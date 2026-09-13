import type { Metadata } from "next";
import localFont from "next/font/local";
import { ThemeProvider } from "@/components/theme-provider";
import { SettingsProvider } from "@/contexts/settings-context";
import { I18nProvider } from "@/contexts/i18n-context";
import { Toaster } from "@/components/ui/toaster";
import { LocalBridge } from "@/components/local-bridge";
import { AppBootstrap } from "@/components/app-bootstrap";
import "./globals.css";

// Inter embarquée (sous-ensemble latin, graisses 100 à 900) : le build ne
// dépend plus de fonts.googleapis.com, ce qui le rend reproductible et
// possible hors ligne (VM de build macOS et Linux). Fichier issu de Google
// Fonts, licence OFL.
const inter = localFont({
  src: "./fonts/inter-latin-variable.woff2",
  weight: "100 900",
  display: "swap",
});

export const metadata: Metadata = {
  title: "ZedSuite",
  description: "Open source ECU map editor (Bosch EDC15/EDC16)",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className={inter.className}>
        <LocalBridge>
          <ThemeProvider
            attribute="class"
            defaultTheme="dark"
            enableSystem
            disableTransitionOnChange
          >
            <SettingsProvider>
              <I18nProvider>
                {children}
                <AppBootstrap />
                <Toaster />
              </I18nProvider>
            </SettingsProvider>
          </ThemeProvider>
        </LocalBridge>
      </body>
    </html>
  );
}
