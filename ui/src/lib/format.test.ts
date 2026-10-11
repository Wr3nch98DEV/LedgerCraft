import { describe, it, expect } from 'vitest';
import { money, parseMoney, fixed, qty, date, addMonths, monthEnd, moneyPlain } from './format';

describe('money formatting', () => {
  it('formats cents with grouping', () => {
    expect(money(123456789)).toBe('1,234,567.89');
    expect(money(-505)).toBe('-5.05');
    expect(money(0)).toBe('0.00');
    expect(money(0, { blankZero: true })).toBe('');
    expect(money(null)).toBe('');
  });
  it('round-trips plain values', () => {
    expect(moneyPlain(-1205)).toBe('-12.05');
    expect(parseMoney(moneyPlain(987654))).toBe(987654);
  });
});

describe('parseMoney', () => {
  it('accepts common inputs', () => {
    expect(parseMoney('1,234.50')).toBe(123450);
    expect(parseMoney('$12')).toBe(1200);
    expect(parseMoney('(12.30)')).toBe(-1230);
    expect(parseMoney('-0.005')).toBe(-1);
    expect(parseMoney('0.004')).toBe(0);
    expect(parseMoney('abc')).toBeNull();
    expect(parseMoney('')).toBeNull();
  });
});

describe('fixed point and dates', () => {
  it('trims fixed decimals', () => {
    expect(fixed(15000, 4)).toBe('1.5');
    expect(qty(10000)).toBe('1');
    expect(fixed(1999900, 4, 2)).toBe('199.99');
  });
  it('handles month arithmetic', () => {
    expect(addMonths('2026-01-31', 1)).toBe('2026-02-28');
    expect(monthEnd('2024-02-10')).toBe('2024-02-29');
    expect(date('2026-10-12')).toBe('12 Oct 2026');
  });
});
