interface IconProps {
  size?: number;
  className?: string;
}

export const ChevronDownIcon = ({ size = 12, className = '' }: IconProps) => (
  <svg width={size} height={size * (8 / 12)} viewBox="0 0 12 8" fill="none" className={className}>
    <path
      d="M1 1.5L6 6.5L11 1.5"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);
