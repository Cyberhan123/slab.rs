import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import {
  Table,
  TableHeader,
  TableBody,
  TableFooter,
  TableHead,
  TableRow,
  TableCell,
  TableCaption,
} from "@/table"
import { renderComponentScene } from "../test-utils"

function TableGallery() {
  return (
    <div data-testid="table-gallery" className="rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <Table data-testid="table">
        <TableCaption>A list of your recent invoices</TableCaption>
        <TableHeader>
          <TableRow>
            <TableHead>Invoice</TableHead>
            <TableHead>Status</TableHead>
            <TableHead>Amount</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow>
            <TableCell>INV001</TableCell>
            <TableCell>Paid</TableCell>
            <TableCell>$250.00</TableCell>
          </TableRow>
          <TableRow>
            <TableCell>INV002</TableCell>
            <TableCell>Pending</TableCell>
            <TableCell>$150.00</TableCell>
          </TableRow>
          <TableRow>
            <TableCell>INV003</TableCell>
            <TableCell>Unpaid</TableCell>
            <TableCell>$350.00</TableCell>
          </TableRow>
        </TableBody>
        <TableFooter>
          <TableRow>
            <TableCell colSpan={3}>Total: $750.00</TableCell>
          </TableRow>
        </TableFooter>
      </Table>
    </div>
  )
}

describe("Table browser coverage", () => {
  it("matches the shared table gallery screenshot", async () => {
    await renderComponentScene(<TableGallery />)
    const table = page.getByTestId("table")
    await expect.element(table).toBeVisible()
    await expect(page.getByTestId("table-gallery")).toMatchScreenshot("table-gallery.png")
  })

  it("keeps interactive table structure covered", async () => {
    await renderComponentScene(
      <Table data-testid="test-table">
        <TableHeader data-testid="table-header">
          <TableRow data-testid="table-row">
            <TableHead>Name</TableHead>
            <TableHead>Status</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody data-testid="table-body">
          <TableRow data-testid="table-row">
            <TableCell>Item 1</TableCell>
            <TableCell>Active</TableCell>
          </TableRow>
        </TableBody>
      </Table>
    )
    const table = page.getByTestId("test-table")
    await expect.element(table).toBeVisible()

    const header = page.getByTestId("table-header")
    await expect.element(header).toBeVisible()

    const body = page.getByTestId("table-body")
    await expect.element(body).toBeVisible()

    const firstRow = page.getByTestId("table-row").first()
    const secondRow = page.getByTestId("table-row").nth(1)
    await expect.element(firstRow).toBeVisible()
    await expect.element(secondRow).toBeVisible()
  })
})
