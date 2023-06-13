import { Maybe } from "../util/types";
import { DialogPromiseFactory } from "../context/modal";
import { useState } from "react";
import { Button, DialogActions, DialogContentText, DialogTitle, TextField } from "@mui/material";

export function stringDialog(createDialogPromise: DialogPromiseFactory, settings: {
  title: string,
  placeholder: string,
  action: string,
  default?: string,
}) {
  return createDialogPromise<Maybe<string>>(resolve => {
    const [name, setName] = useState(settings.default ?? "");
    return <>
      <DialogTitle>
        { settings.title }
      </DialogTitle>
      <DialogContentText sx={{ml: "1rem", mr: "1rem"}}>
        <TextField variant="standard" placeholder={ settings.action } value={name} onChange={ e => setName(e.target.value) }/>
      </DialogContentText>
      <DialogActions>
        <Button onClick={() => resolve(null)}>Cancel</Button>
        <Button onClick={() => resolve(name)}>{ settings.action }</Button>
      </DialogActions>
    </>
  })
}
