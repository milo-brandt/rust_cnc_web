import { PropsWithChildren, createContext, useContext, useEffect, useState } from "react";
import { Maybe } from "../util/types";
import { Button, Dialog, DialogActions, DialogContent, DialogTitle } from "@mui/material";

export interface Dialog {
  title: string,
  message: JSX.Element,
  actions: string[],
}

type DialogCallback = (snack: Dialog) => Promise<string>;

const DialogContext = createContext<DialogCallback | null>(null)

export function useDialog() {
  const createDialog = useContext(DialogContext)!;
  return {
    createDialog,
  }
}

export interface DialogWithPromise extends Dialog {
  resolve?: (value: string) => void;
}

export function DialogProvider({ children }: PropsWithChildren<{}>) {
  const [dialog, setDialog] = useState<Maybe<DialogWithPromise>>(null);
  const [open, setOpen] = useState(false);
  // Track whether the component is mounted...
  let isActive = true;
  useEffect(() => () => { isActive = false; }, []);

  const handleClose = (_?: any, reason?: string) => {
    if (reason === 'backdropClick') {
      return;
    }
    setOpen(false);
  };

  return (
    <DialogContext.Provider value={dialog => {
      if (isActive) {
        setOpen(true);
        return new Promise(resolve => setDialog({
          ...dialog,
          resolve
        }));
      } else {
        return Promise.reject("dialog context expired");
      }
    }}>
      { children }
      <Dialog open={open && dialog !== null} onClose={handleClose}>
        <DialogTitle>
          { dialog?.title }
        </DialogTitle>
        <DialogContent>
          {dialog?.message}
        </DialogContent>
        <DialogActions>
          {
            dialog?.actions?.map(action => {
              return (<Button onClick={
                () => {
                  dialog?.resolve?.(action);
                  dialog.resolve = undefined;
                  handleClose();
                }
              }>{ action }</Button>)
            })
          }
        </DialogActions>
      </Dialog>
    </DialogContext.Provider>
  )
}