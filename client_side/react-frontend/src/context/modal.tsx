import { PropsWithChildren, createContext, useContext, useEffect, useState } from "react";
import { Maybe } from "../util/types";
import { Dialog } from "@mui/material";

export interface Dialog {
  title: string,
  message: JSX.Element,
  actions: string[],
}

type DialogCallback = (snack: (close: () => void) => JSX.Element) => void;

const DialogContext = createContext<DialogCallback | null>(null)

export type DialogPromiseFactory = <T,>(dialog: (resolve: (value: T) => void) => JSX.Element) => Promise<T>;

export function useDialog() {
  const createDialogRaw = useContext(DialogContext)!;
  return {
    createDialogRaw,
    createDialogPromise: <T,>(dialog: (resolve: (value: T) => void) => JSX.Element): Promise<T> => new Promise(
      resolve => {
        let isResolvable = true;
        console.log("CREATING RAW DIALOG!!!")
        createDialogRaw(close => dialog((value) => {
          if(isResolvable) {
            close();
            resolve(value);
          }
        }))
      }
    )
  }
}


export function DialogProvider({ children }: PropsWithChildren<{}>) {
  const [dialog, setDialog] = useState<Maybe<JSX.Element>>(null);
  const [open, setOpen] = useState(false);
  // Track whether the component is mounted...
  let isActive = true;
  useEffect(() => () => { isActive = false; }, []);

  /*const handleClose = (_?: any, reason?: string) => {
    if (reason === 'backdropClick') {
      return;
    }
    setOpen(false);
  };*/

  return (
    <DialogContext.Provider value={dialogFactory => {
      if (isActive) {
        setOpen(true);
        let localActive = true;
        const close = () => {
          if(isActive && localActive) {
            localActive = false;
            setOpen(false);
          }
        };
        // Wrap the dialog in a component so it can have state.
        function Dialog() {
          return dialogFactory(close);
        }
        setDialog(<Dialog/>)
      }
    }}>
      { children }
      <Dialog open={open && dialog !== null}>
        { dialog }
      </Dialog>
    </DialogContext.Provider>
  )
}

/*
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

*/