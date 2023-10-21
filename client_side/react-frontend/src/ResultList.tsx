import { useResultListing } from "./api/files";
import { Link } from "react-router-dom";
import { PageLoading, PageErrored } from "./ErrorState";
import { IconButton, Paper, Table, TableBody, TableCell, TableContainer, TableRow, Tooltip, Typography } from "@mui/material";
import { Download } from "@mui/icons-material";
import { HOST } from "./api/constants";


export default function ResultListPage() {
  const { result: directoryListingResult } = useResultListing();
  if(directoryListingResult.status == "loading") {
    return <PageLoading/>
  } else if(directoryListingResult.status == "rejected") {
    return <PageErrored/>
  } else {
    const directories = directoryListingResult.data;
    const fileItems = directories.map(item => {
      return (
        <TableRow key={item}>
          <TableCell>
            { item }
          </TableCell>
          <TableCell>
            <Tooltip title="Download">
              <IconButton component={Link} to={`http://${HOST}/results/download/${item}`}><Download /></IconButton>
            </Tooltip>
          </TableCell>
        </TableRow>
      )
    });
    return (
      <div>
        <Typography variant="h4">
          Results
        </Typography>
        <Paper>
          <TableContainer>
            <Table>
              <TableBody>
                {
                  fileItems
                }
              </TableBody>
            </Table>
          </TableContainer>
        </Paper>
      </div>
    )
  }
}