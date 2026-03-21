interface StatusBarProps {
  message: string;
  busy: boolean;
  isError?: boolean;
}

export function StatusBar({ message, busy, isError }: StatusBarProps) {
  return (
    <div className="flex flex-col items-center gap-3">
      <p
        className={`text-center px-6 leading-relaxed ${isError ? "text-error" : "text-on-surface"}`}
        style={{ fontSize: "0.875rem" }}
      >
        {message}
      </p>
      {busy && (
        <div className="w-56 h-0.5 bg-surface-container-high rounded-full overflow-hidden">
          <div className="h-full bg-primary rounded-full animate-[indeterminate_1.5s_ease-in-out_infinite]" />
        </div>
      )}
    </div>
  );
}
