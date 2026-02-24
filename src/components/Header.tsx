import type { SessionInfo } from '@/types';

interface HeaderProps {
  sessions: SessionInfo[];
  onRefresh: () => void;
  onToggleNotifications?: () => void;
}

export const Header = ({ sessions, onRefresh, onToggleNotifications }: HeaderProps) => {
  const waitingCount = sessions.filter(
    (s) => s.status === 'WaitingPermission' || s.status === 'WaitingInput'
  ).length;

  const isWaiting = waitingCount > 0;

  return (
    <header className="flex flex-col gap-1.5 pb-1.5 shrink-0">
      <div className="flex justify-between items-center border-b border-bg-card py-1.5 flex-nowrap">
        <div className="flex items-center gap-3">
          <h1 className="font-semibold text-sm whitespace-nowrap">Eyes on Claude Code</h1>
        </div>
        <div className="flex items-center gap-2 bg-bg-card rounded-full py-0.5 px-2 text-[0.625rem] whitespace-nowrap shrink-0">
          <div
            className={`w-2 h-2 rounded-full bg-success ${isWaiting ? 'bg-warning animate-pulse-slow' : ''}`}
          />
          <span>{isWaiting ? `${waitingCount} waiting` : 'Monitoring'}</span>
        </div>
      </div>
      <div className="flex justify-between items-center">
        <h2 className="font-semibold text-xs">Sessions</h2>
        <div className="flex gap-1">
          {onToggleNotifications && (
            <button
              onClick={onToggleNotifications}
              className="bg-bg-card border-none text-text-primary rounded-lg cursor-pointer transition-colors hover:bg-accent py-0.5 px-2 text-[0.625rem]"
              title="Notification settings"
            >
              Notify
            </button>
          )}
          <button
            onClick={onRefresh}
            className="bg-bg-card border-none text-text-primary rounded-lg cursor-pointer transition-colors hover:bg-accent py-0.5 px-2 text-[0.625rem]"
          >
            Refresh
          </button>
        </div>
      </div>
    </header>
  );
};
