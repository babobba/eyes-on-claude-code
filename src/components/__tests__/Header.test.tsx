import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Header } from '../Header';
import type { SessionInfo } from '@/types';

const makeSession = (status: SessionInfo['status']): SessionInfo => ({
  project_name: `project-${status}`,
  project_dir: `/home/user/${status}`,
  status,
  last_event: '2025-01-01T00:00:00Z',
  waiting_for: '',
  tmux_pane: '',
});

describe('Header', () => {
  it('renders title', () => {
    render(<Header sessions={[]} onRefresh={() => {}} />);
    expect(screen.getByText('Eyes on Claude Code')).toBeInTheDocument();
  });

  it('shows "Monitoring" when no sessions are waiting', () => {
    const sessions = [makeSession('Active'), makeSession('Completed')];
    render(<Header sessions={sessions} onRefresh={() => {}} />);
    expect(screen.getByText('Monitoring')).toBeInTheDocument();
  });

  it('shows waiting count when sessions are in WaitingPermission', () => {
    const sessions = [makeSession('WaitingPermission'), makeSession('Active')];
    render(<Header sessions={sessions} onRefresh={() => {}} />);
    expect(screen.getByText('1 waiting')).toBeInTheDocument();
  });

  it('shows waiting count for WaitingInput sessions', () => {
    const sessions = [makeSession('WaitingInput')];
    render(<Header sessions={sessions} onRefresh={() => {}} />);
    expect(screen.getByText('1 waiting')).toBeInTheDocument();
  });

  it('counts both WaitingPermission and WaitingInput', () => {
    const sessions = [makeSession('WaitingPermission'), makeSession('WaitingInput')];
    render(<Header sessions={sessions} onRefresh={() => {}} />);
    expect(screen.getByText('2 waiting')).toBeInTheDocument();
  });

  it('shows "Monitoring" with empty sessions', () => {
    render(<Header sessions={[]} onRefresh={() => {}} />);
    expect(screen.getByText('Monitoring')).toBeInTheDocument();
  });

  it('calls onRefresh when Refresh button is clicked', async () => {
    const user = userEvent.setup();
    const onRefresh = vi.fn();
    render(<Header sessions={[]} onRefresh={onRefresh} />);

    await user.click(screen.getByText('Refresh'));
    expect(onRefresh).toHaveBeenCalledTimes(1);
  });

  it('renders Sessions heading', () => {
    render(<Header sessions={[]} onRefresh={() => {}} />);
    expect(screen.getByText('Sessions')).toBeInTheDocument();
  });
});
