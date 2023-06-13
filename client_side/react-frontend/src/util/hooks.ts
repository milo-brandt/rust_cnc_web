import { useState } from "react";

export function useLocalStorage<T>(key: string, initialValue: () => T): [T, (value: T) => void] {
  const [currentValue, setValueRaw] = useState(() => {
    const result = localStorage.getItem(key);
    if(result === null) {
      return initialValue();
    } else {
      return JSON.parse(result);
    }
  });
  return [currentValue, (value: T) => {
    setValueRaw(value);
    localStorage.setItem(key, JSON.stringify(value));
  }]
}