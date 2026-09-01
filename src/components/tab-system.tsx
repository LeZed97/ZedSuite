"use client";

import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useEffect, useRef, useState } from "react";

export interface Tab {
  id: string;
  title: string;
  type: "hexdump" | "map";
  closeable: boolean;
  data?: any;
}

interface TabSystemProps {
  tabs: Tab[];
  activeTabId: string;
  onTabChange: (tabId: string) => void;
  onTabClose: (tabId: string) => void;
  onScrollbarChange?: (hasScrollbar: boolean) => void;
}

export function TabSystem({ tabs, activeTabId, onTabChange, onTabClose, onScrollbarChange }: TabSystemProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const activeTabRef = useRef<HTMLDivElement>(null);
  const [hasScrollbar, setHasScrollbar] = useState(false);

  // Check if scrollbar is visible
  useEffect(() => {
    const checkScrollbar = () => {
      if (containerRef.current) {
        const hasHorizontalScroll = containerRef.current.scrollWidth > containerRef.current.clientWidth;
        setHasScrollbar(hasHorizontalScroll);
        onScrollbarChange?.(hasHorizontalScroll);
      }
    };

    checkScrollbar();
    // Check on resize
    window.addEventListener('resize', checkScrollbar);
    // Use MutationObserver to detect when tabs change
    const observer = new MutationObserver(checkScrollbar);
    if (containerRef.current) {
      observer.observe(containerRef.current, { childList: true, subtree: true });
    }

    return () => {
      window.removeEventListener('resize', checkScrollbar);
      observer.disconnect();
    };
  }, [tabs, onScrollbarChange]);

  // Scroll to active tab when it changes
  useEffect(() => {
    if (activeTabRef.current && containerRef.current) {
      const container = containerRef.current;
      const activeTab = activeTabRef.current;
      
      const containerRect = container.getBoundingClientRect();
      const tabRect = activeTab.getBoundingClientRect();
      
      // Check if tab is outside visible area
      if (tabRect.left < containerRect.left) {
        // Tab is to the left, scroll it into view
        activeTab.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'start' });
      } else if (tabRect.right > containerRect.right) {
        // Tab is to the right, scroll it into view
        activeTab.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'end' });
      }
    }
  }, [activeTabId]);

  return (
    <div 
      ref={containerRef}
      className={`flex items-end gap-0 px-2 pt-1 ${hasScrollbar ? 'pb-0.5' : ''} overflow-hidden`}
      style={{ 
        minWidth: 'max-content',
        maxHeight: '100%'
      }}
    >
      {tabs.map((tab) => (
        <div
          key={tab.id}
          ref={activeTabId === tab.id ? activeTabRef : null}
          className={`
            flex items-center gap-2 px-3 py-2 min-w-[150px] max-w-[220px] cursor-pointer
            rounded-t-lg transition-all relative flex-shrink-0
            ${
              activeTabId === tab.id
                ? "bg-[#2a2a2a] text-white shadow-lg shadow-black/20 z-20"
                : "bg-[#1a1a1a] text-white/60 hover:bg-[#252525] hover:text-white/80 z-0"
            }
          `}
          style={activeTabId === tab.id ? { 
            marginBottom: '-1px',
            borderTop: '1px solid rgba(255, 255, 255, 0.2)',
            borderLeft: '1px solid rgba(255, 255, 255, 0.2)',
            borderRight: '1px solid rgba(255, 255, 255, 0.2)',
            borderBottom: 'none'
          } : {}}
          onClick={() => onTabChange(tab.id)}
        >
          <span className="flex-1 truncate text-xs font-medium">{tab.title}</span>
          {tab.closeable && (
            <Button
              variant="ghost"
              size="sm"
              className="h-4 w-4 p-0 hover:bg-white/10 rounded flex-shrink-0"
              onClick={(e) => {
                e.stopPropagation();
                onTabClose(tab.id);
              }}
            >
              <X className="w-3 h-3" />
            </Button>
          )}
        </div>
      ))}
    </div>
  );
}
