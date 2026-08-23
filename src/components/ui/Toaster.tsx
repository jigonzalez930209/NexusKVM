import { Toaster as SonnerToaster } from 'sonner';

export function Toaster() {
  return (
    <SonnerToaster
      position="top-right"
      theme="dark"
      offset={16}
      gap={8}
      visibleToasts={4}
      toastOptions={{
        unstyled: true,
      }}
    />
  );
}
