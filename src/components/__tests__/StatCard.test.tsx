import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { StatCard } from '../StatCard';

describe('StatCard', () => {
  it('renders value and label', () => {
    render(<StatCard value={42} label="Active" />);
    expect(screen.getByText('42')).toBeInTheDocument();
    expect(screen.getByText('Active')).toBeInTheDocument();
  });

  it('renders zero value', () => {
    render(<StatCard value={0} label="Waiting" />);
    expect(screen.getByText('0')).toBeInTheDocument();
    expect(screen.getByText('Waiting')).toBeInTheDocument();
  });
});
