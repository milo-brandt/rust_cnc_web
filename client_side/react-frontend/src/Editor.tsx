import { Link, useParams } from "react-router-dom";
import CodeMirror, { KeyBinding } from "@uiw/react-codemirror";
import { keymap, EditorView } from "@codemirror/view"
import { defaultKeymap } from "@codemirror/commands"
import { useCancellablePromise } from "./api/generic";
import { useMemo, useState } from "react";
import { cncAxios } from "./api/cncAxios";
import { uploadFile } from "./api/files";
import { IconButton } from "@mui/material";
import { ArrowBack, Save } from "@mui/icons-material";

export default function EditorPage() {
  let { "*": directory } = useParams() as {"*": string};
  if(directory && directory[directory.length - 1] == "/") {
    directory = directory.slice(0, directory.length - 1);
  }
  const parent = (() => {
    const lastSlash = directory.lastIndexOf("/");
    if(lastSlash === -1) {
      return ""
    } else {
      return directory.slice(0, lastSlash);
    }
  })();
  const [lastSaved, setLastSaved] = useState<null | string>(null);
  const [currentValue, setCurrentValue] = useState<null | string>(null);
  const result = 
    useCancellablePromise(
      useMemo(
        () => async (signal: AbortSignal) => {
          // await new Promise(resolve => setTimeout(resolve, 500));
          let result = await cncAxios.get<string>(`/job/download_file/${directory}`, { signal, transformResponse: x => x });
          return result.data;
        },
        []
      )
    );
  async function save(text: string) {
    setLastSaved(text);
    await uploadFile(directory, new Blob([text], { type: 'text/plain' }))
  }
  const hasNewContent = result.status == 'resolved' && currentValue != lastSaved;
  const myKeymap: KeyBinding[] = [
    ...defaultKeymap,
    {
      key: 'Ctrl-1',
      mac: 'Cmd-1',
      run: (view) => {
        save(view.state.doc.toString())
        return true;
      }
    }
  ]
  let content: JSX.Element;
  if (result.status == 'loading') {
    content = <div>Loading contents...</div>
  } else if (result.status == 'rejected') {
    content = <div>Failed to load contents.</div>
  } else {
    if(lastSaved === null) {
      setLastSaved(result.data)
      setCurrentValue(result.data)
    }
    content = <CodeMirror
      value={result.data}
      extensions={
        [
          keymap.of(myKeymap),
          EditorView.theme({
            "&": {
              fontSize: "0.9rem",
            }
          })
        ]
      }
      onChange={ (value) => setCurrentValue(value) }
    />
  }
  return <div>
    <IconButton color="inherit" size="large" component={ Link } to={ `/gcode/${parent}` }>
      <ArrowBack color="inherit" sx={{
        fontSize: "2rem"
      }}/>
    </IconButton>
    <IconButton color="inherit" size="large" onClick={ () => save(currentValue!) } disabled={ !hasNewContent }>
      <Save color="inherit" sx={{
        fontSize: "2rem"
      }}/>
    </IconButton>
    { content }
  </div>
}