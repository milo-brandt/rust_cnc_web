import { DirectoryItem, createDirectory, deleteFile, executeFile, uploadFile, useDirectoryListing } from "./api/files";
import { Link, useParams } from "react-router-dom";
import { PageLoading, PageErrored } from "./ErrorState";
import { flatten, groupBy } from "lodash";
import { Box, Button, DialogContentText, IconButton, Paper, TableCell, TableContainer, TableRow, TextField, Tooltip, Typography } from "@mui/material";
import { ArrowUpward, CreateNewFolder, Delete, Download, Folder, MoveUp, PlayArrow, Search, UploadFile } from "@mui/icons-material";
import { useStatusContext } from "./context/status";
import { useSnackbar } from "./context/snackbar";
import { useDialog } from "./context/modal";
import { SyntheticEvent, useState } from "react";
import { Maybe } from "./util/types";


function getParentDirectory(directory: string): Maybe<string> {
  // Trim trailing slash if present
  if (directory[directory.length - 1] == "/") {
    directory = directory.slice(0, directory.length - 1);
  }
  // Root directory has no parent...
  if (directory.length == 0) {
    return null;
  }

  const lastSlash = directory.lastIndexOf("/");
  if (lastSlash == -1) {
    return "";
  } else {
    return directory.slice(0, lastSlash);
  }
}

function FileUpload({setValue}: {setValue: (value: string) => void}) {
  const [name, setName] = useState("");
  setValue(name);
  return <DialogContentText>
    <TextField variant="standard" placeholder="Directory Name" value={name} onChange={ e => setName(e.target.value) }/>
  </DialogContentText>
}

export function FileListPage() {
  const { "*": directory } = useParams() as {"*": string};
  const pathPrefix = directory ? directory + "/" : "";
  const { result: directoryListingResult, reload: reloadDirectories } = useDirectoryListing(directory);
  const { jobStatus } = useStatusContext();
  const canRunJob = jobStatus === null;
  const { createSnack, snackAsyncCatch } = useSnackbar();
  const requestFileDeletion = (path: string) => snackAsyncCatch(deleteFile(path), () => `Failed to delete ${path}`).then(() => reloadDirectories());
  const { createDialog } = useDialog(); 

  async function uploadFiles(event: SyntheticEvent) {
    const input = event.target as HTMLInputElement;
    const promises: Array<Promise<string[]>> = [];
    const fileCount = input.files?.length ?? 0;
    for(const file of input.files ?? []) {
      const filename = file.name;
      promises.push(uploadFile(`${pathPrefix}${filename}`, file).then(() => [], () => [filename]));
    }
    let failed = flatten(await Promise.all(promises));
    if (failed.length > 0) {
      createSnack({
        message: `Failed to upload files: ${ failed.join(", ") }`,
        severity: `error`,
      })
    } else if(fileCount > 0) {
      createSnack({
        message: `Uploaded ${fileCount} files.`,
        severity: "success"
      })
    }
    reloadDirectories();
  }
  async function directoryCreation() {
    let value: Maybe<string> = null;
    let choice = await createDialog({
      title: "Create Folder",
      message: <FileUpload setValue={ x => { value = x; } }/>,
      actions: ["Cancel", "Create"]
    });
    if(choice == "Create" && value) {
      await snackAsyncCatch(createDirectory(pathPrefix + value), () => `Failed to create directory ${value}`).finally(reloadDirectories);
    }
  }

  if(directoryListingResult.status == "loading") {
    return <PageLoading/>
  } else if(directoryListingResult.status == "rejected") {
    return <PageErrored/>
  } else {
    const directoryListing = directoryListingResult.data;
    const { false: directories = [], true: files = [] } = groupBy(directoryListing, (item: DirectoryItem) => item.is_file);
    const directoryItems = directories.map(item => {
      const itemPath = pathPrefix + item.name;
      return (
        <TableRow key={item.name}>
          <TableCell>
            <Link to={ `/gcode/${pathPrefix}${item.name}` }>
              <Box display="flex" alignItems="center">
              <Folder sx={{ mr: 1 }}/>
              {item.name}
              </Box>
            </Link>
          </TableCell>
          <TableCell>
            <Tooltip title="Delete">
              <IconButton onClick={ async () => {
                let choice = await createDialog({
                  title: "Confirm folder deletion?",
                  message: <DialogContentText>{`Delete the folder /${ itemPath } and all of its contents.`}</DialogContentText>,
                  actions: ["Cancel", "Delete"]
                });
                if(choice == "Delete") {
                  await requestFileDeletion(itemPath);
                }
               }}>
                <Delete/>
              </IconButton>
            </Tooltip>
          </TableCell>
        </TableRow>
      )  
    });
    const fileItems = files.map(item => {
      const itemPath = pathPrefix + item.name;
      return (
        <TableRow key={item.name}>
          <TableCell>
          <Folder sx={{ mr: 1, visibility: "hidden" }}/>
            { item.name }
          </TableCell>
          <TableCell>
            <Tooltip title="Run">
              <IconButton onClick={() => snackAsyncCatch(executeFile(itemPath), () => `Failed to run ${itemPath}`)} disabled={!canRunJob}><PlayArrow/></IconButton>
            </Tooltip>
            <Tooltip title="Inspect">
              <IconButton component={Link} to={`/view/${itemPath}/`}><Search /></IconButton>
            </Tooltip>
            <Tooltip title="Download">
              <IconButton component={Link} to={`http://localhost:3000/job/download_file/${itemPath}`}><Download /></IconButton>
            </Tooltip>
            <Tooltip title="Delete">
              <IconButton onClick={ () => requestFileDeletion(itemPath) }>
                <Delete/>
              </IconButton>
            </Tooltip>
          </TableCell>
        </TableRow>
      )
    });
    const placeholderItem = [];
    if(directoryItems.length === 0 && fileItems.length === 0) {
      placeholderItem.push(
        <TableRow key="placeholder">
          <TableCell sx={{fontStyle: "italic"}}>
            <Folder sx={{ mr: 1, visibility: "hidden" }}/>
            This folder is empty.
          </TableCell>
          <TableCell>

          </TableCell>
        </TableRow>
      )
    }
    return (
      <div>
        <Typography variant="h4">
          <IconButton
            component={Link}
            to={ `/gcode/${getParentDirectory(directory) ?? ""}` }
            sx={{ mr: 2, visibility: directory ? "visible" : "hidden" }}
          >
          <ArrowUpward/>
          </IconButton>
          { directory + "/" }
        </Typography>
        <Paper>
          <TableContainer component="table">
            <tbody>
              {
                [...directoryItems, ...fileItems, ...placeholderItem]
              }
              <TableRow>
                <TableCell>
                <Button variant="contained" startIcon={ <CreateNewFolder/> } sx={{mr: 1}} onClick={() => directoryCreation()}>Create Folder</Button>
                <Button variant="contained" startIcon={ <UploadFile/> } component="label">Upload<input type="file" hidden multiple onChange={ uploadFiles }/></Button>
                </TableCell>
                <TableCell>

                </TableCell>
              </TableRow>
            </tbody>
          </TableContainer>
        </Paper>
      </div>
    )
  }
}