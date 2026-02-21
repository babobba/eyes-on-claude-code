import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { SessionList } from '../SessionList';
import type { SessionInfo } from '@/types';

const makeSession = (
  name: string,
  status: SessionInfo['status'] = 'Active'
): SessionInfo => ({
  project_name: name,
  project_dir: `/home/user/${name}`,
  status,
  last_event: '2025-01-01T00:00:00Z',
  waiting_for: '',
  tmux_pane: '',
});

describe('SessionList', () => {
  it('renders empty state when no sessions', () => {
    render(<SessionList sessions={[]} />);
    expect(screen.getByText('No active sessions')).toBeInTheDocument();
    expect(screen.getByText('📭')).toBeInTheDocument();
  });

  it('renders session cards for each session', () => {
    const sessions = [makeSession('project-a'), makeSession('project-b', 'Completed')];
    render(<SessionList sessions={sessions} />);
    expect(screen.getByText('project-a')).toBeInTheDocument();
    expect(screen.getByText('project-b')).toBeInTheDocument();
  });

  it('renders correct status emojis for sessions', () => {
    const sessions = [
      makeSession('active-proj', 'Active'),
      makeSession('waiting-proj', 'WaitingPermission'),
    ];
    render(<SessionList sessions={sessions} />);
    expect(screen.getByText('🟢')).toBeInTheDocument();
    expect(screen.getByText('🔐')).toBeInTheDocument();
  });
});
