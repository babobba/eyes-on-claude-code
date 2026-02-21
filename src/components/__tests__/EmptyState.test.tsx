import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { EmptyState } from '../EmptyState';

describe('EmptyState', () => {
  it('renders icon and message', () => {
    render(<EmptyState icon="📭" message="No active sessions" />);
    expect(screen.getByText('📭')).toBeInTheDocument();
    expect(screen.getByText('No active sessions')).toBeInTheDocument();
  });

  it('renders different icon and message', () => {
    render(<EmptyState icon="🔍" message="Nothing found" />);
    expect(screen.getByText('🔍')).toBeInTheDocument();
    expect(screen.getByText('Nothing found')).toBeInTheDocument();
  });
});
