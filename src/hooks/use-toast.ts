import { useState, useEffect, useSyncExternalStore } from "react"

type ToastProps = {
  id: string
  title?: string
  description?: string
  action?: React.ReactNode
  variant?: "default" | "destructive"
}

// Global toast store so Toaster and any component share the same toasts
let globalToasts: ToastProps[] = []
const listeners = new Set<() => void>()

function notify() {
  listeners.forEach((l) => l())
}

function addToast(props: Omit<ToastProps, "id">) {
  // Prevent duplicate toasts from stacking
  const isDuplicate = globalToasts.some(
    (t) => t.title === props.title && t.description === props.description
  )
  if (isDuplicate) return

  const id = Math.random().toString(36).substr(2, 9)
  globalToasts = [...globalToasts, { id, ...props }]
  notify()
  setTimeout(() => {
    globalToasts = globalToasts.filter((t) => t.id !== id)
    notify()
  }, 4000)
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => { listeners.delete(listener) }
}

function getSnapshot() {
  return globalToasts
}

export function useToast() {
  const toasts = useSyncExternalStore(subscribe, getSnapshot, getSnapshot)

  return { toasts, toast: addToast }
}
