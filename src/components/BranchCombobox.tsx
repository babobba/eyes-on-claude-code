import { useState, useRef, useEffect, useCallback } from 'react';
import { ChevronDownIcon } from './icons';

interface BranchComboboxProps {
  branches: string[];
  value: string;
  onSelect: (branch: string) => void;
}

export const BranchCombobox = ({ branches, value, onSelect }: BranchComboboxProps) => {
  const [isOpen, setIsOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [openUpward, setOpenUpward] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

  const filtered = search
    ? branches.filter((b) => b.toLowerCase().includes(search.toLowerCase()))
    : branches;

  const checkDirection = useCallback(() => {
    if (!buttonRef.current) return;
    const rect = buttonRef.current.getBoundingClientRect();
    const dropdownHeight = 180;
    const spaceBelow = window.innerHeight - rect.bottom;
    setOpenUpward(spaceBelow < dropdownHeight);
  }, []);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setIsOpen(false);
        setSearch('');
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  useEffect(() => {
    if (isOpen) {
      checkDirection();
      inputRef.current?.focus();
    }
  }, [isOpen, checkDirection]);

  return (
    <div ref={containerRef} className="relative">
      <button
        ref={buttonRef}
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-1 text-[0.625rem] text-text-secondary hover:text-white px-1.5 py-0.5 bg-bg-card rounded hover:bg-white/10 transition-colors max-w-[140px]"
      >
        <span className="truncate">{value}</span>
        <ChevronDownIcon
          size={8}
          className={`shrink-0 transition-transform ${isOpen ? 'rotate-180' : ''}`}
        />
      </button>

      {isOpen && (
        <div
          className={`absolute z-50 left-0 w-48 bg-bg-card border border-white/10 rounded shadow-lg shadow-black/40 ${
            openUpward ? 'bottom-full mb-1' : 'top-full mt-1'
          }`}
        >
          <div className="p-1">
            <input
              ref={inputRef}
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search branches..."
              className="w-full text-[0.625rem] bg-bg-primary text-text-primary px-2 py-1 rounded border border-white/10 outline-none focus:border-info/50 placeholder:text-text-secondary"
            />
          </div>
          <div className="max-h-32 overflow-y-auto">
            {filtered.length === 0 ? (
              <div className="text-[0.625rem] text-text-secondary px-2 py-1">No branches found</div>
            ) : (
              filtered.map((branch) => (
                <button
                  key={branch}
                  onClick={() => {
                    onSelect(branch);
                    setIsOpen(false);
                    setSearch('');
                  }}
                  className={`w-full text-left text-[0.625rem] px-2 py-1 hover:bg-white/10 transition-colors truncate ${
                    branch === value ? 'text-info' : 'text-text-primary'
                  }`}
                >
                  {branch}
                </button>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
};
