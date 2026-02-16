import React from "react";

export function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

type ButtonVariant = "primary" | "ghost" | "danger" | "subtle";
type ButtonSize = "sm" | "md";

export function Button({
  variant = "subtle",
  size = "md",
  className,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: ButtonSize;
}) {
  return (
    <button
      className={cx(
        "tr-btn",
        `tr-btn--${variant}`,
        `tr-btn--${size}`,
        props.disabled && "tr-btn--disabled",
        className,
      )}
      {...props}
    />
  );
}

export function Input({
  className,
  ...props
}: React.InputHTMLAttributes<HTMLInputElement> & { className?: string }) {
  return <input className={cx("tr-input", className)} {...props} />;
}

export function Select({
  className,
  ...props
}: React.SelectHTMLAttributes<HTMLSelectElement> & { className?: string }) {
  return <select className={cx("tr-input", className)} {...props} />;
}

export function TextArea({
  className,
  ...props
}: React.TextareaHTMLAttributes<HTMLTextAreaElement> & { className?: string }) {
  return <textarea className={cx("tr-input tr-textarea", className)} {...props} />;
}

export function Card({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cx("tr-card", className)} {...props} />;
}

export function Badge({
  tone = "neutral",
  className,
  ...props
}: React.HTMLAttributes<HTMLSpanElement> & { tone?: "neutral" | "good" | "warn" | "bad" | "info" }) {
  return <span className={cx("tr-badge", `tr-badge--${tone}`, className)} {...props} />;
}

export function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <label className="tr-field">
      <div className="tr-field__top">
        <div className="tr-field__label">{label}</div>
        {hint ? <div className="tr-field__hint">{hint}</div> : null}
      </div>
      {children}
    </label>
  );
}

