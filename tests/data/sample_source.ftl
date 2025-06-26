# This file tests preservation of comments and variables during conversion

# Simple string without variables
app-title = My Application

# String with single variable
welcome-user = Welcome back, {$username}!

# String with multiple variables  
user-info = User {$name} has {$email} and joined on {$date}

# Plural forms with variables
notification-count = {$count ->
    [one] You have {$count} new notification
   *[other] You have {$count} new notifications
}

# Multi-line string with indentation
# with a multi-line comment
# as comments also are being parsed
long-description = This is a longer description that spans
    multiple lines to test how multi-line content
    is preserved during the round trip conversion
    process.

# String with special characters and formatting
formatted-price = Price: ${$amount} USD (includes {$tax}% tax)

# String with leading spaces (edge case)
spaced-message =     This message starts with spaces and has {$variable}

# Another comment in the middle
# Testing comment preservation

# String with special characters in variable names
database-query = Query executed in {$execution_time_ms}ms returning {$row_count} rows

# Multi-line with variables
email-template = Dear {$recipient_name},
    
    Thank you for your order #{$order_id}.
    Your {$item_count} items will be shipped to {$address}.
    
    Best regards,
    {$company_name}

# test empty key
empty-key = ""

block =
    Sometimes it's more readable to format
    multiline text as a "block", which means
    starting it on a new line. All lines must
    be indented by at least one space.

leading-spaces =     This message's value starts with the word "This".
leading-lines =


    This message's value starts with the word "This".
    The blank lines under the identifier are ignored.

# Final comment at the end
