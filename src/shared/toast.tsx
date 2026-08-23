import React from 'react';
import { toast as sonnerToast } from 'sonner';
import {
  CheckCircle2,
  AlertCircle,
  Info,
  AlertTriangle,
  X,
} from 'lucide-react';

interface ToastOptions {
  description?: string;
  duration?: number;
}

export const toast = {
  success: (message: string, descriptionOrOptions?: string | ToastOptions) => {
    const desc =
      typeof descriptionOrOptions === 'string'
        ? descriptionOrOptions
        : descriptionOrOptions?.description;
    const duration =
      typeof descriptionOrOptions === 'object'
        ? descriptionOrOptions.duration
        : 3500;

    return sonnerToast.custom(
      (id) => (
        <div className="w-full max-w-sm bg-surface-container/95 backdrop-blur-md border border-outline-variant shadow-2xl shadow-black rounded-xl p-3.5 flex items-start gap-3 text-on-surface font-sans select-none animate-in fade-in slide-in-from-top-2 duration-200">
          <div className="w-8 h-8 rounded-lg bg-tertiary/10 border border-tertiary/20 flex items-center justify-center shrink-0 text-tertiary">
            <CheckCircle2 size={16} />
          </div>
          <div className="flex-1 min-w-0 pt-0.5">
            <div className="font-semibold text-xs text-on-surface leading-tight">
              {message}
            </div>
            {desc && (
              <div className="text-[11px] text-on-surface-variant mt-1 leading-normal">
                {desc}
              </div>
            )}
          </div>
          <button
            onClick={() => sonnerToast.dismiss(id)}
            className="text-on-surface-variant hover:text-on-surface p-1 rounded transition-colors cursor-pointer shrink-0"
            aria-label="Dismiss notification"
          >
            <X size={13} />
          </button>
        </div>
      ),
      { duration },
    );
  },

  error: (message: string, descriptionOrOptions?: string | ToastOptions) => {
    const desc =
      typeof descriptionOrOptions === 'string'
        ? descriptionOrOptions
        : descriptionOrOptions?.description;
    const duration =
      typeof descriptionOrOptions === 'object'
        ? descriptionOrOptions.duration
        : 4500;

    return sonnerToast.custom(
      (id) => (
        <div className="w-full max-w-sm bg-surface-container/95 backdrop-blur-md border border-error/30 shadow-2xl shadow-black rounded-xl p-3.5 flex items-start gap-3 text-on-surface font-sans select-none animate-in fade-in slide-in-from-top-2 duration-200">
          <div className="w-8 h-8 rounded-lg bg-error/10 border border-error/20 flex items-center justify-center shrink-0 text-error">
            <AlertCircle size={16} />
          </div>
          <div className="flex-1 min-w-0 pt-0.5">
            <div className="font-semibold text-xs text-on-surface leading-tight">
              {message}
            </div>
            {desc && (
              <div className="text-[11px] text-on-surface-variant mt-1 leading-normal">
                {desc}
              </div>
            )}
          </div>
          <button
            onClick={() => sonnerToast.dismiss(id)}
            className="text-on-surface-variant hover:text-on-surface p-1 rounded transition-colors cursor-pointer shrink-0"
            aria-label="Dismiss notification"
          >
            <X size={13} />
          </button>
        </div>
      ),
      { duration },
    );
  },

  warning: (message: string, descriptionOrOptions?: string | ToastOptions) => {
    const desc =
      typeof descriptionOrOptions === 'string'
        ? descriptionOrOptions
        : descriptionOrOptions?.description;
    const duration =
      typeof descriptionOrOptions === 'object'
        ? descriptionOrOptions.duration
        : 4000;

    return sonnerToast.custom(
      (id) => (
        <div className="w-full max-w-sm bg-surface-container/95 backdrop-blur-md border border-yellow-500/30 shadow-2xl shadow-black rounded-xl p-3.5 flex items-start gap-3 text-on-surface font-sans select-none animate-in fade-in slide-in-from-top-2 duration-200">
          <div className="w-8 h-8 rounded-lg bg-yellow-500/10 border border-yellow-500/20 flex items-center justify-center shrink-0 text-yellow-500">
            <AlertTriangle size={16} />
          </div>
          <div className="flex-1 min-w-0 pt-0.5">
            <div className="font-semibold text-xs text-on-surface leading-tight">
              {message}
            </div>
            {desc && (
              <div className="text-[11px] text-on-surface-variant mt-1 leading-normal">
                {desc}
              </div>
            )}
          </div>
          <button
            onClick={() => sonnerToast.dismiss(id)}
            className="text-on-surface-variant hover:text-on-surface p-1 rounded transition-colors cursor-pointer shrink-0"
            aria-label="Dismiss notification"
          >
            <X size={13} />
          </button>
        </div>
      ),
      { duration },
    );
  },

  info: (message: string, descriptionOrOptions?: string | ToastOptions) => {
    const desc =
      typeof descriptionOrOptions === 'string'
        ? descriptionOrOptions
        : descriptionOrOptions?.description;
    const duration =
      typeof descriptionOrOptions === 'object'
        ? descriptionOrOptions.duration
        : 3500;

    return sonnerToast.custom(
      (id) => (
        <div className="w-full max-w-sm bg-surface-container/95 backdrop-blur-md border border-outline-variant shadow-2xl shadow-black rounded-xl p-3.5 flex items-start gap-3 text-on-surface font-sans select-none animate-in fade-in slide-in-from-top-2 duration-200">
          <div className="w-8 h-8 rounded-lg bg-primary/10 border border-primary/20 flex items-center justify-center shrink-0 text-primary">
            <Info size={16} />
          </div>
          <div className="flex-1 min-w-0 pt-0.5">
            <div className="font-semibold text-xs text-on-surface leading-tight">
              {message}
            </div>
            {desc && (
              <div className="text-[11px] text-on-surface-variant mt-1 leading-normal">
                {desc}
              </div>
            )}
          </div>
          <button
            onClick={() => sonnerToast.dismiss(id)}
            className="text-on-surface-variant hover:text-on-surface p-1 rounded transition-colors cursor-pointer shrink-0"
            aria-label="Dismiss notification"
          >
            <X size={13} />
          </button>
        </div>
      ),
      { duration },
    );
  },
};
