import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MinimumView } from '../MinimumView';
import type { SessionInfo } from '@/types';

const makeSession = (status: SessionInfo['status']): SessionInfo => ({
  project_name: `project-${status}`,
  project_dir: `/home/user/${status}`,
  status,
  last_event: '2025-01-01T00:00:00Z',
  waiting_for: '',
  tmux_pane: '',
});

describe('MinimumView', () => {
  it('renders title', () => {
    render(<MinimumView sessions={[]} />);
    expect(screen.getByText('EoCC')).toBeInTheDocument();
  });

  it('shows "Idle" when there are no sessions', () => {
    render(<MinimumView sessions={[]} />);
    expect(screen.getByText('Idle')).toBeInTheDocument();
  });

  it('shows status counts for active sessions', () => {
    const sessions = [makeSession('Active'), makeSession('Active')];
    render(<MinimumView sessions={sessions} />);
    expect(screen.getByText(/🟢:2/)).toBeInTheDocument();
  });

  it('shows counts for multiple statuses', () => {
    const sessions = [
      makeSession('Active'),
      makeSession('WaitingPermission'),
      makeSession('WaitingInput'),
      makeSession('Completed'),
    ];
    render(<MinimumView sessions={sessions} />);
    expect(screen.getByText(/🟢:1/)).toBeInTheDocument();
    expect(screen.getByText(/🔐:1/)).toBeInTheDocument();
    expect(screen.getByText(/⏳:1/)).toBeInTheDocument();
    expect(screen.getByText(/✅:1/)).toBeInTheDocument();
  });

  it('hides statuses with zero count', () => {
    const sessions = [makeSession('Active')];
    render(<MinimumView sessions={sessions} />);
    expect(screen.getByText(/🟢:1/)).toBeInTheDocument();
    expect(screen.queryByText(/🔐/)).not.toBeInTheDocument();
    expect(screen.queryByText(/⏳/)).not.toBeInTheDocument();
    expect(screen.queryByText(/✅/)).not.toBeInTheDocument();
  });
});
