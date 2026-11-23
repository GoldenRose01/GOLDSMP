import { useEffect, useState } from 'react';

export type RamUnit = 'MB' | 'GB';

const STORAGE_KEY = 'goldsmp_ram_unit';
const EVENT_NAME = 'goldsmp-ram-unit-change';

export function getRamUnit(): RamUnit {
  const saved = localStorage.getItem(STORAGE_KEY);
  return saved === 'GB' ? 'GB' : 'MB';
}

export function setRamUnitGlobal(unit: RamUnit) {
  localStorage.setItem(STORAGE_KEY, unit);
  window.dispatchEvent(new CustomEvent<RamUnit>(EVENT_NAME, { detail: unit }));
}

export function toggleRamUnitGlobal(): RamUnit {
  const next: RamUnit = getRamUnit() === 'MB' ? 'GB' : 'MB';
  setRamUnitGlobal(next);
  return next;
}

export function useRamUnit(): [RamUnit, (u: RamUnit) => void, () => void] {
  const [unit, setUnit] = useState<RamUnit>(() => getRamUnit());

  useEffect(() => {
    const handler = (e: Event) => {
      const custom = e as CustomEvent<RamUnit>;
      setUnit(custom.detail || getRamUnit());
    };
    window.addEventListener(EVENT_NAME, handler);
    return () => window.removeEventListener(EVENT_NAME, handler);
  }, []);

  return [
    unit,
    (u: RamUnit) => setRamUnitGlobal(u),
    () => {
      toggleRamUnitGlobal();
    },
  ];
}

export function formatRamValue(mb: number, unit: RamUnit, decimals?: number): string {
  if (unit === 'GB') {
    const gb = mb / 1024;
    const dec = decimals !== undefined ? decimals : gb >= 10 ? 1 : 2;
    return `${gb.toFixed(dec)} GB`;
  }
  const dec = decimals !== undefined ? decimals : 0;
  return `${mb.toFixed(dec)} MB`;
}

export function formatRamPair(usedMb: number, maxMb: number, unit: RamUnit): string {
  if (unit === 'GB') {
    const usedGb = usedMb / 1024;
    const maxGb = maxMb / 1024;
    const maxStr = Number.isInteger(maxGb) ? maxGb.toFixed(0) : maxGb.toFixed(2);
    return `${usedGb.toFixed(2)} / ${maxStr} GB`;
  }
  return `${usedMb.toFixed(0)} / ${maxMb.toFixed(0)} MB`;
}

export function mbToUnitNumber(mb: number, unit: RamUnit): number {
  if (unit === 'GB') {
    return Number((mb / 1024).toFixed(2));
  }
  return Math.round(mb);
}

export function unitNumberToMb(val: number, unit: RamUnit): number {
  if (unit === 'GB') {
    return Math.max(128, Math.round(val * 1024));
  }
  return Math.max(128, Math.round(val));
}
