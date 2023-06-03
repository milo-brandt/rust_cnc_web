import { Alert, Snackbar } from "@mui/material";
import { PropsWithChildren, createContext, useContext, useEffect, useState } from "react";
import { Maybe } from "../util/types";

export interface Snack {
  message: string,
  severity: "success" | "warning" | "error" | "info"
}

type SnackCallback = (snack: Snack) => void;

const SnackBarContext = createContext<SnackCallback | null>(null)

export function useSnackbar() {
  const createSnack = useContext(SnackBarContext)!;
  return {
    createSnack,
    snackAsyncCatch: <T,>(promise: Promise<T>, onError: (error: any) => string) => promise.catch(
      err => {
        createSnack({
          message: onError(err),
          severity: "error",
        });
        throw err;
      }
    )
  }
}

export function SnackBarProvider({ children }: PropsWithChildren<{}>) {
  const [snack, setSnack] = useState<Maybe<Snack>>(null);
  const [open, setOpen] = useState(false);
  // Track whether the component is mounted...
  let isActive = true;
  useEffect(() => () => { isActive = false; }, []);

  const handleClose = (_?: any, reason?: string) => {
    if (reason === 'clickaway') {
      return;
    }
    setOpen(false);
  };

  return (
    <SnackBarContext.Provider value={snack => {
      if (isActive) {
        setOpen(true);
        setSnack(snack);
      }
    }}>
      { children }
      <Snackbar
        open={open && snack !== null} 
        autoHideDuration={6000}
        onClose={handleClose}
      >
        <Alert severity={snack?.severity}>
          {snack?.message}
        </Alert>
      </Snackbar>
    </SnackBarContext.Provider>
  )
}