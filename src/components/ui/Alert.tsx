import React from 'react';
import { AlertCircle, CheckCircle2, Info, AlertTriangle } from 'lucide-react';

export interface AlertProps extends React.HTMLAttributes<HTMLDivElement> {
  variant?: 'default' | 'destructive' | 'success' | 'warning';
  title?: string;
  children?: React.ReactNode;
}

export function Alert({
  variant = 'default',
  title,
  children,
  className = '',
  ...props
}: AlertProps) {
  const iconMap = {
    default: <Info size={16} className="text-primary shrink-0 mt-0.5" />,
    destructive: (
      <AlertCircle size={16} className="text-error shrink-0 mt-0.5" />
    ),
    success: (
      <CheckCircle2 size={16} className="text-tertiary shrink-0 mt-0.5" />
    ),
    warning: (
      <AlertTriangle size={16} className="text-yellow-500 shrink-0 mt-0.5" />
    ),
  };

  const variantStyles = {
    default: 'border-outline-variant bg-surface-container text-on-surface',
    destructive:
      'border-error/40 bg-error/10 text-on-surface [&>svg]:text-error',
    success:
      'border-tertiary/40 bg-tertiary/10 text-on-surface [&>svg]:text-tertiary',
    warning:
      'border-yellow-500/40 bg-yellow-500/10 text-on-surface [&>svg]:text-yellow-500',
  };

  return (
    <div
      role="alert"
      className={`relative w-full rounded-lg border p-4 flex items-start gap-3 text-sm shadow-sm transition-all ${variantStyles[variant]} ${className}`}
      {...props}
    >
      {iconMap[variant]}
      <div className="flex flex-col gap-1 flex-1 min-w-0">
        {title && (
          <h5 className="font-semibold text-xs text-on-surface leading-tight">
            {title}
          </h5>
        )}
        <div className="text-xs text-on-surface-variant leading-relaxed">
          {children}
        </div>
      </div>
    </div>
  );
}
