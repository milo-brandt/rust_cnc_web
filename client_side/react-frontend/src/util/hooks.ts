import { useCallback, useEffect, useRef } from "react";

export function useIsActive() {
  const result = useRef(true);
  useEffect(() => () => { result.current = false; }, []);
  return useCallback(() => result.current, []);
}