import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { DiffButton } from '../DiffButton';

describe('DiffButton', () => {
  it('renders with Diff text', () => {
    render(<DiffButton onClick={() => {}} />);
    expect(screen.getByText('Diff')).toBeInTheDocument();
  });

  it('calls onClick when clicked', async () => {
    const user = userEvent.setup();
    const onClick = vi.fn();
    render(<DiffButton onClick={onClick} />);

    await user.click(screen.getByText('Diff'));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('applies small styling when small prop is true', () => {
    render(<DiffButton onClick={() => {}} small />);
    const button = screen.getByText('Diff').closest('button');
    expect(button).toHaveClass('px-1.5');
  });

  it('applies custom className', () => {
    render(<DiffButton onClick={() => {}} className="shrink-0" />);
    const button = screen.getByText('Diff').closest('button');
    expect(button).toHaveClass('shrink-0');
  });
});
