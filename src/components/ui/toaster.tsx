"use client"

import { useEffect, useState } from "react"
import { Check, AlertTriangle } from "lucide-react"
import { useToast } from "@/hooks/use-toast"
import { TOAST_GLASS } from "@/lib/modal-glass"

function ToastItem({ title, description, action, variant }: {
  id: string
  title?: string
  description?: string
  action?: React.ReactNode
  variant?: "default" | "destructive"
}) {
  const [phase, setPhase] = useState<"enter" | "visible" | "exit">("enter")

  useEffect(() => {
    requestAnimationFrame(() => setPhase("visible"))
    const timer = setTimeout(() => setPhase("exit"), 3500)
    return () => clearTimeout(timer)
  }, [])

  const isDestructive = variant === "destructive"

  return (
    <div
      className="fixed bottom-6 z-[100] flex items-center gap-2.5 px-3 py-1.5 rounded-full text-white pointer-events-auto"
      style={{
        ...TOAST_GLASS,
        // Centré sur l'ÉCRAN (pas sur l'espace de travail de l'éditeur) —
        // même référence pour toutes les vignettes du bas de l'app.
        left: '50%',
        transform: 'translateX(-50%)',
        animation: phase === "exit"
          ? "toastFadeOut 0.5s ease-out forwards"
          : phase === "visible"
          ? "toastSlideIn 0.3s ease-out forwards"
          : "none",
        opacity: phase === "enter" ? 0 : undefined,
      }}
    >
      {/* Tinted status disc, same language as the modal icons */}
      <span
        className="w-6 h-6 rounded-full flex items-center justify-center flex-shrink-0"
        style={{ backgroundColor: isDestructive ? 'rgba(239, 68, 68, 0.15)' : 'rgba(34, 197, 94, 0.15)' }}
      >
        {isDestructive ? (
          <AlertTriangle className="w-3.5 h-3.5" style={{ color: '#ef4444' }} />
        ) : (
          <Check className="w-3.5 h-3.5" style={{ color: '#22c55e' }} />
        )}
      </span>
      <div className="flex flex-col pr-1">
        {title && <span className="font-medium text-sm">{title}</span>}
        {description && <span className="text-xs text-white/60">{description}</span>}
      </div>
      {action}
    </div>
  )
}

export function Toaster() {
  const { toasts } = useToast()

  return (
    <>
      {toasts.map((toast) => (
        <ToastItem key={toast.id} {...toast} />
      ))}
    </>
  )
}
